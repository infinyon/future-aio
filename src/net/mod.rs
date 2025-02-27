#[cfg(not(target_arch = "wasm32"))]
pub use async_net::*;

#[cfg(not(target_arch = "wasm32"))]
pub mod tcp_stream;

pub use conn::*;

#[cfg(not(target_arch = "wasm32"))]
pub use unix_connector::DefaultTcpDomainConnector as DefaultDomainConnector;

#[cfg(target_arch = "wasm32")]
pub use wasm_connector::DefaultDomainWebsocketConnector as DefaultDomainConnector;

mod conn {

    use async_trait::async_trait;
    use futures_lite::io::{AsyncRead, AsyncWrite};
    use std::io::Error as IoError;

    pub trait Connection: AsyncRead + AsyncWrite + Send + Sync + Unpin + SplitConnection {}
    impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin + SplitConnection> Connection for T {}

    pub trait ReadConnection: AsyncRead + Send + Sync + Unpin {}
    impl<T: AsyncRead + Send + Sync + Unpin> ReadConnection for T {}

    pub trait WriteConnection: AsyncWrite + Send + Sync + Unpin {}
    impl<T: AsyncWrite + Send + Sync + Unpin> WriteConnection for T {}

    pub type BoxConnection = Box<dyn Connection>;
    pub type BoxReadConnection = Box<dyn ReadConnection>;
    pub type BoxWriteConnection = Box<dyn WriteConnection>;

    pub trait SplitConnection {
        // split into write and read
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection);
    }

    cfg_if::cfg_if! {
        if #[cfg(unix)] {
            pub type ConnectionFd = std::os::unix::io::RawFd;
            pub trait AsConnectionFd: std::os::unix::io::AsRawFd {
                fn as_connection_fd(&self) -> ConnectionFd {
                    self.as_raw_fd()
                }
            }
            impl AsConnectionFd for async_net::TcpStream { }
        } else if #[cfg(windows)] {
            pub type ConnectionFd = std::os::windows::io::RawSocket;
            pub trait AsConnectionFd: std::os::windows::io::AsRawSocket {
                fn as_connection_fd(&self) -> ConnectionFd {
                    self.as_raw_socket()
                }
            }
            impl AsConnectionFd for async_net::TcpStream { }
        } else {
            pub type ConnectionFd = String;
        }
    }

    pub type DomainConnector = Box<dyn TcpDomainConnector>;

    impl Clone for DomainConnector {
        fn clone(&self) -> DomainConnector {
            self.clone_box()
        }
    }

    pub trait AsyncConnector: Send + Sync {}

    impl<T: Send + Sync> AsyncConnector for T {}

    /// connect to domain and return connection
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    pub trait TcpDomainConnector: AsyncConnector {
        async fn connect(
            &self,
            domain: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError>;

        // create new version of my self with new domain
        fn new_domain(&self, domain: String) -> DomainConnector;

        fn domain(&self) -> &str;

        fn clone_box(&self) -> DomainConnector;
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod certs {

    use std::fs::File;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::path::Path;

    use anyhow::Result;
    use tracing::debug;

    pub trait CertBuilder: Sized {
        fn new(bytes: Vec<u8>) -> Self;

        fn from_reader(reader: &mut dyn BufRead) -> Result<Self> {
            let mut bytes = vec![];
            reader.read_to_end(&mut bytes)?;
            Ok(Self::new(bytes))
        }

        fn from_path(path: impl AsRef<Path>) -> Result<Self> {
            debug!("loading cert from: {}", path.as_ref().display());
            let mut reader = BufReader::new(File::open(path)?);
            Self::from_reader(&mut reader)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_connector {
    use super::*;
    use async_trait::async_trait;
    use futures_util::io::AsyncReadExt;
    use std::io::Error as IoError;
    use ws_stream_wasm::WsMeta;

    #[derive(Clone, Default)]
    pub struct DefaultDomainWebsocketConnector {}
    impl DefaultDomainWebsocketConnector {
        pub fn new() -> Self {
            Self {}
        }
    }
    #[async_trait(?Send)]
    impl TcpDomainConnector for DefaultDomainWebsocketConnector {
        async fn connect(
            &self,
            addr: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError> {
            let (mut _ws, wsstream) = WsMeta::connect(addr, None)
                .await
                .map_err(|e| IoError::new(std::io::ErrorKind::Other, e))?;
            let wsstream_io = wsstream.into_io();
            let (stream, sink) = wsstream_io.split();
            Ok((Box::new(sink), Box::new(stream), String::from(addr)))
        }

        fn new_domain(&self, _domain: String) -> DomainConnector {
            Box::new(self.clone())
        }

        fn domain(&self) -> &str {
            "localhost"
        }

        fn clone_box(&self) -> DomainConnector {
            Box::new(self.clone())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod unix_connector {
    use async_trait::async_trait;
    use std::io::Error as IoError;
    use tracing::debug;

    use super::tcp_stream::stream;

    use super::*;

    impl SplitConnection for TcpStream {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            (Box::new(self.clone()), Box::new(self))
        }
    }

    /// creates TcpStream connection
    #[derive(Clone, Default)]
    pub struct DefaultTcpDomainConnector {}

    impl DefaultTcpDomainConnector {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl TcpDomainConnector for DefaultTcpDomainConnector {
        async fn connect(
            &self,
            addr: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError> {
            debug!("connect to tcp addr: {}", addr);
            let tcp_stream = stream(addr).await?;

            let fd = tcp_stream.as_connection_fd();
            Ok((Box::new(tcp_stream.clone()), Box::new(tcp_stream), fd))
        }

        fn new_domain(&self, _domain: String) -> DomainConnector {
            Box::new(self.clone())
        }

        fn domain(&self) -> &str {
            "localhost"
        }

        fn clone_box(&self) -> DomainConnector {
            Box::new(self.clone())
        }
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod test {
    use std::time;

    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_util::AsyncReadExt;
    use tracing::debug;

    use crate::net::tcp_stream::stream;
    use crate::net::TcpListener;
    use crate::test_async;
    use crate::timer::sleep;

    use super::*;

    #[test_async]
    async fn test_clone() -> Result<(), ()> {
        let addr = format!("127.0.0.1:{}", 39000)
            .parse::<SocketAddr>()
            .expect("parse");

        let server_ft = async {
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await.expect("listener failed");
            debug!("server: successfully binding. waiting for incoming");

            let mut incoming = listener.incoming();
            let incoming_stream = incoming.next().await.expect("incoming");

            debug!("server: got connection from client");
            let _ = incoming_stream.expect("no stream");

            // sleep 1 seconds so we don't lost connection
            sleep(time::Duration::from_secs(1)).await;

            Ok(()) as Result<(), ()>
        };

        let client_ft = async {
            sleep(time::Duration::from_millis(100)).await;
            let tcp_stream = stream(&addr).await.expect("test");
            let (_read, _write) = tcp_stream.split();
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod test {
    use super::*;
    use futures_util::{AsyncReadExt, AsyncWriteExt};
    use wasm_bindgen_test::*;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    #[wasm_bindgen_test]
    async fn test_connect() {
        tracing_wasm::set_as_global_default();

        let addr = "ws://127.0.0.1:1234";
        let input_msg = "foobar".to_string();

        let websocket_stream = DefaultDomainConnector::default();
        let (mut writer, mut reader, _id) = websocket_stream.connect(addr).await.expect("test");

        writer
            .write(input_msg.as_bytes())
            .await
            .expect("Failed to write");

        let mut output = vec![0; input_msg.len()];
        let size = reader.read(&mut output).await.expect("Failed to read");
        assert_eq!(output, input_msg.as_bytes());
        assert_eq!(size, input_msg.len());
    }
}
