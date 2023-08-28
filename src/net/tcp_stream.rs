use std::time::Duration;

use async_net::AsyncToSocketAddrs;

use socket2::SockRef;
use socket2::TcpKeepalive;
use tracing::debug;

use crate::net::TcpStream;

/// This setting determines the time (in seconds) that a connection must be idle before the first keepalive packet is sent.
/// The default value is 7200 seconds, or 2 hours. This means that if there is no data exchange on a connection for 2 hours,
/// the system will send a keepalive packet to the remote host to check if the connection is still active
const TCP_KEEPALIVE_TIME: Duration = Duration::from_secs(7200);
/// This setting specifies the interval (in seconds) between successive keepalive packets if no response (acknowledgment)
/// is received from the remote host. The default value is 75 seconds. If the first keepalive packet does not receive a response,
/// the system will send additional keepalive packets every 75 seconds until it receives a response or reaches the maximum number
/// of allowed probes (as defined by tcp_keepalive_probes).
const TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(75);
/// This setting defines the maximum number of unacknowledged keepalive packets that the system will send before considering the connection dead.
/// The default value is 9 probes. If the system sends 9 keepalive packets without receiving a response,
/// it assumes the connection is dead and closes it.
#[cfg(not(windows))]
const TCP_KEEPALIVE_PROBES: u32 = 9;

#[derive(Debug, Clone, Default)]
pub struct SocketOpts {
    pub nodelay: Option<bool>,
    pub keepalive: Option<KeepaliveOpts>,
}

#[derive(Debug, Clone)]
pub struct KeepaliveOpts {
    pub time: Option<Duration>,
    pub interval: Option<Duration>,
    #[cfg(not(windows))]
    pub retries: Option<u32>,
}

#[cfg(not(windows))]
impl Default for KeepaliveOpts {
    fn default() -> Self {
        Self {
            time: Some(TCP_KEEPALIVE_TIME),
            interval: Some(TCP_KEEPALIVE_INTERVAL),
            retries: Some(TCP_KEEPALIVE_PROBES),
        }
    }
}

#[cfg(windows)]
impl Default for KeepaliveOpts {
    fn default() -> Self {
        Self {
            time: Some(TCP_KEEPALIVE_TIME),
            interval: Some(TCP_KEEPALIVE_INTERVAL),
        }
    }
}

impl From<&KeepaliveOpts> for TcpKeepalive {
    fn from(value: &KeepaliveOpts) -> Self {
        let mut result = TcpKeepalive::new();
        if let Some(time) = value.time {
            result = result.with_time(time);
        }
        if let Some(interval) = value.interval {
            result = result.with_interval(interval);
        }
        cfg_if::cfg_if! {
            if #[cfg(not(windows))] {
                if let Some(retries) = value.retries {
                    result = result.with_retries(retries);
                }
            }
        }
        result
    }
}

pub async fn stream<A: AsyncToSocketAddrs>(addr: A) -> Result<TcpStream, std::io::Error> {
    let socket_opts = SocketOpts {
        keepalive: Some(Default::default()),
        ..Default::default()
    };
    stream_with_opts(addr, Some(socket_opts)).await
}

pub async fn stream_with_opts<A: AsyncToSocketAddrs>(
    addr: A,
    socket_opts: Option<SocketOpts>,
) -> Result<TcpStream, std::io::Error> {
    debug!(?socket_opts);
    let tcp_stream = TcpStream::connect(addr).await?;
    if let Some(socket_opts) = socket_opts {
        let socket_ref = SockRef::from(&tcp_stream);
        if let Some(nodelay) = socket_opts.nodelay {
            socket_ref.set_nodelay(nodelay)?;
        }
        if let Some(ref keepalive_opts) = socket_opts.keepalive {
            let keepalive = TcpKeepalive::from(keepalive_opts);
            socket_ref.set_tcp_keepalive(&keepalive)?;
        }
    }
    Ok(tcp_stream)
}

#[cfg(test)]
mod tests {
    use std::io::Error;

    use super::*;
    use crate::test_async;
    use crate::timer::sleep;
    use async_net::SocketAddr;
    use async_net::TcpListener;
    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use futures_lite::future::zip;
    use futures_lite::AsyncReadExt;
    use futures_util::SinkExt;
    use futures_util::StreamExt;
    use log::debug;
    use tokio_util::codec::BytesCodec;
    use tokio_util::codec::Framed;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }

    #[test_async]
    async fn test_async_tcp() -> Result<(), Error> {
        let addr = "127.0.0.1:9998".parse::<SocketAddr>().expect("parse");

        let server_ft = async {
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await?;
            debug!("server: successfully binding. waiting for incoming");
            let mut incoming = listener.incoming();
            let stream = incoming.next().await.expect("no stream");
            debug!("server: got connection from client");
            let tcp_stream = stream?;
            let mut framed = Framed::new(tcp_stream.compat(), BytesCodec::new());
            debug!("server: sending values to client");
            let data = vec![0x05, 0x0a, 0x63];
            framed.send(to_bytes(data)).await?;
            Ok(()) as Result<(), Error>
        };

        let client_ft = async {
            debug!("client: sleep to give server chance to come up");
            sleep(Duration::from_millis(100)).await;
            debug!("client: trying to connect");
            let tcp_stream = stream(&addr).await?;
            let mut framed = Framed::new(tcp_stream.compat(), BytesCodec::new());
            debug!("client: got connection. waiting");
            let value = framed.next().await.expect("no value received");
            debug!("client :received first value from server");
            let bytes = value?;
            debug!("client :received bytes len: {}", bytes.len());
            assert_eq!(bytes.len(), 3);
            let values = bytes.take(3).into_inner();
            assert_eq!(values[0], 0x05);
            assert_eq!(values[1], 0x0a);
            assert_eq!(values[2], 0x63);

            Ok(()) as Result<(), Error>
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }

    #[test_async]
    async fn test_tcp_stream_socket_opts() -> Result<(), Error> {
        let addr = "127.0.0.1:9997".parse::<SocketAddr>().expect("parse");

        let server_ft = async {
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await?;
            debug!("server: successfully binding. waiting for incoming");
            let mut incoming = listener.incoming();
            let _stream = incoming.next().await.expect("no stream");
            let _stream = incoming.next().await.expect("no stream");
            debug!("server: got connection from client");
            Ok(()) as Result<(), Error>
        };

        let client_ft = async {
            debug!("client: sleep to give server chance to come up");
            sleep(Duration::from_millis(100)).await;
            debug!("client: trying to connect");
            {
                let socket_opts = SocketOpts {
                    keepalive: None,
                    nodelay: Some(false),
                };
                let tcp_stream = stream_with_opts(&addr, Some(socket_opts)).await?;
                assert!(!tcp_stream.nodelay()?);
                let socket_ref = SockRef::from(&tcp_stream);
                assert!(!(socket_ref.nodelay()?));
                assert!(!(socket_ref.keepalive()?));
            }
            {
                let time = Duration::from_secs(7201);
                let interval = Duration::from_secs(76);
                cfg_if::cfg_if! {
                    if #[cfg(windows)] {
                        let socket_opts = SocketOpts {
                            keepalive: Some(KeepaliveOpts {
                                time: Some(time),
                                interval: Some(interval),
                            }),
                            nodelay: Some(true),
                        };
                    } else {
                        let retries = 10;
                        let socket_opts = SocketOpts {
                            keepalive: Some(KeepaliveOpts {
                                time: Some(time),
                                interval: Some(interval),
                                retries: Some(retries),
                            }),
                            nodelay: Some(true),
                        };
                    }
                }

                let tcp_stream = stream_with_opts(&addr, Some(socket_opts)).await?;
                assert!(tcp_stream.nodelay()?);
                let socket_ref = SockRef::from(&tcp_stream);
                assert!(socket_ref.nodelay()?);
                assert!(socket_ref.keepalive()?);
                cfg_if::cfg_if! {
                    if #[cfg(not(windows))] {
                        assert_eq!(socket_ref.keepalive_time()?, time);
                        assert_eq!(socket_ref.keepalive_interval()?, interval);
                        assert_eq!(socket_ref.keepalive_retries()?, retries);
                    }
                }
            }

            Ok(()) as Result<(), Error>
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
