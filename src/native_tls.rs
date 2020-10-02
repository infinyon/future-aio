use crate::net::TcpStream;

pub use async_native_tls::TlsAcceptor;
pub use async_native_tls::TlsConnector;
pub use async_native_tls::TlsStream;

pub type DefaultTlsStream = TlsStream<TcpStream>;

pub use connector::*;

mod connector {

    use std::io::Error as IoError;

    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;
    #[cfg(unix)]
    use std::os::unix::io::RawFd;

    use async_native_tls::Error as NativeTlsError;
    use async_trait::async_trait;
    use log::debug;

    use crate::net::DefaultTcpDomainConnector;
    use crate::net::TcpDomainConnector;

    use super::AllTcpStream;
    use super::DefaultTlsStream;
    use super::TcpStream;
    use super::TlsConnector;

    pub enum TlsError {
        Io(IoError),
        Native(NativeTlsError),
    }

    impl From<IoError> for TlsError {
        fn from(error: IoError) -> Self {
            Self::Io(error)
        }
    }

    impl From<NativeTlsError> for TlsError {
        fn from(error: NativeTlsError) -> TlsError {
            Self::Native(error)
        }
    }

    /// connect as anonymous client
    pub struct TlsAnonymousConnector(TlsConnector);

    impl From<TlsConnector> for TlsAnonymousConnector {
        fn from(connector: TlsConnector) -> Self {
            Self(connector)
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsAnonymousConnector {
        type WrapperStream = DefaultTlsStream;
        type Error = TlsError;

        async fn connect(&self, domain: &str) -> Result<(Self::WrapperStream, RawFd), Self::Error> {
            let tcp_stream = TcpStream::connect(domain).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((self.0.connect(domain, tcp_stream).await?, fd))
        }
    }

    pub struct TlsDomainConnector {
        domain: String,
        connector: TlsConnector,
    }

    impl TlsDomainConnector {
        pub fn new(connector: TlsConnector, domain: String) -> Self {
            Self { domain, connector }
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsDomainConnector {
        type WrapperStream = DefaultTlsStream;
        type Error = TlsError;

        async fn connect(&self, addr: &str) -> Result<(Self::WrapperStream, RawFd), Self::Error> {
            debug!("connect to tls addr: {}", addr);
            let tcp_stream = TcpStream::connect(addr).await?;
            let fd = tcp_stream.as_raw_fd();

            debug!("connect to tls domain: {}", self.domain);
            Ok((self.connector.connect(&self.domain, tcp_stream).await?, fd))
        }
    }

    pub enum AllDomainConnector {
        Tcp(DefaultTcpDomainConnector),
        TlsDomain(TlsDomainConnector),
        TlsAnonymous(TlsAnonymousConnector),
    }

    impl Default for AllDomainConnector {
        fn default() -> Self {
            Self::default_tcp()
        }
    }

    impl AllDomainConnector {
        pub fn default_tcp() -> Self {
            Self::Tcp(DefaultTcpDomainConnector::new())
        }

        pub fn new_tls_domain(connector: TlsDomainConnector) -> Self {
            Self::TlsDomain(connector)
        }

        pub fn new_tls_anonymous(connector: TlsAnonymousConnector) -> Self {
            Self::TlsAnonymous(connector)
        }
    }

    #[async_trait]
    impl TcpDomainConnector for AllDomainConnector {
        type WrapperStream = AllTcpStream;
        type Error = TlsError;

        async fn connect(&self, domain: &str) -> Result<(Self::WrapperStream, RawFd), Self::Error> {
            match self {
                Self::Tcp(connector) => {
                    let (stream, fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tcp(stream), fd))
                }

                Self::TlsDomain(connector) => {
                    let (stream, fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tls(stream), fd))
                }
                Self::TlsAnonymous(connector) => {
                    let (stream, fd) = connector.connect(domain).await?;
                    Ok((AllTcpStream::tls(stream), fd))
                }
            }
        }
    }
}

pub use stream::AllTcpStream;

mod stream {

    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::DefaultTlsStream;
    use super::TcpStream;
    use futures_lite::{AsyncRead, AsyncWrite};
    use pin_project::pin_project;

    #[pin_project(project = EnumProj)]
    pub enum AllTcpStream {
        Tcp(#[pin] TcpStream),
        Tls(#[pin] DefaultTlsStream),
    }

    impl AllTcpStream {
        pub fn tcp(stream: TcpStream) -> Self {
            Self::Tcp(stream)
        }

        pub fn tls(stream: DefaultTlsStream) -> Self {
            Self::Tls(stream)
        }
    }

    impl AsyncRead for AllTcpStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            match self.project() {
                EnumProj::Tcp(stream) => stream.poll_read(cx, buf),
                EnumProj::Tls(stream) => stream.poll_read(cx, buf),
            }
        }
    }

    impl AsyncWrite for AllTcpStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context,
            buf: &[u8],
        ) -> Poll<Result<usize, io::Error>> {
            match self.project() {
                EnumProj::Tcp(stream) => stream.poll_write(cx, buf),
                EnumProj::Tls(stream) => stream.poll_write(cx, buf),
            }
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            match self.project() {
                EnumProj::Tcp(stream) => stream.poll_flush(cx),
                EnumProj::Tls(stream) => stream.poll_flush(cx),
            }
        }

        fn poll_close(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Result<(), io::Error>> {
            match self.project() {
                EnumProj::Tcp(stream) => stream.poll_close(cx),
                EnumProj::Tls(stream) => stream.poll_close(cx),
            }
        }
    }
}

#[cfg(test)]
mod test {

    use std::io::Error as IoError;
    use std::net::SocketAddr;
    use std::time;

    use async_native_tls::Identity;
    use async_native_tls::TlsAcceptor;
    use async_native_tls::TlsConnector;
    use ::native_tls::TlsAcceptor as SyncTlsAcceptor;
    use bytes::buf::ext::BufExt;
    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_util::sink::SinkExt;
    use tokio_util::codec::BytesCodec;
    use tokio_util::codec::Framed;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    use crate::test_async;
    use crate::timer::sleep;
    use log::debug;

    use crate::net::TcpListener;
    use crate::net::TcpStream;

    use super::AllTcpStream;

   // const CA_PATH: &'static str = "certs/certs/ca.crt";
    const SERVER_IDENTITY: &'static str = "certs/certs/server.pfx";
    const CLIENT_IDENTITY: &'static str = "certs/certs/client.pfx";

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }

    #[test_async]
    async fn test_native_tls_anonymous() -> Result<(), IoError> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(SERVER_IDENTITY).unwrap();
        let mut pkcs12 = vec![];
        file.read_to_end(&mut pkcs12).unwrap();
        let server_identity = Identity::from_pkcs12(&pkcs12, "test").unwrap();
        let acceptor: TlsAcceptor = SyncTlsAcceptor::new(server_identity).unwrap().into();

        let connector = TlsConnector::new()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);

        test_tls(acceptor.clone(), connector)
            .await
            .expect("no client cert test failed");

        Ok(())
    }


    #[test_async]
    async fn test_native_tls_client() -> Result<(), IoError> {
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(SERVER_IDENTITY).unwrap();
        let mut pkcs12 = vec![];
        file.read_to_end(&mut pkcs12).unwrap();
        let server_identity = Identity::from_pkcs12(&pkcs12, "test").unwrap();
        let acceptor: TlsAcceptor = SyncTlsAcceptor::new(server_identity).unwrap().into();


        let mut file2 = File::open(CLIENT_IDENTITY).unwrap();
        let mut pkcs122 = vec![];
        file2.read_to_end(&mut pkcs122).unwrap();
        let client_identity = Identity::from_pkcs12(&pkcs122, "test").unwrap();

        let connector = TlsConnector::new()
            .identity(client_identity)
            .danger_accept_invalid_certs(true);

        // test client authentication
        
        test_tls(acceptor,connector)
            .await
            .expect("client cert test fail");
    

        Ok(())
    }

    async fn test_tls(acceptor: TlsAcceptor, connector: TlsConnector) -> Result<(), IoError> {
        const TEST_ITERATION: u16 = 2;

        let addr = "127.0.0.1:19998".parse::<SocketAddr>().expect("parse");

        let server_ft = async {
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await.expect("listener failed");
            debug!("server: successfully binding. waiting for incoming");

            let mut incoming = listener.incoming();
            let incoming_stream = incoming.next().await.expect("incoming");

            debug!("server: got connection from client");
            let tcp_stream = incoming_stream.expect("no stream");

            let tls_stream = acceptor.accept(tcp_stream).await.unwrap();
            let mut framed = Framed::new(tls_stream.compat(), BytesCodec::new());

            for _ in 0..TEST_ITERATION { 
                debug!("server: sending values to client");
                let data = vec![0x05, 0x0a, 0x63];
                framed.send(to_bytes(data)).await.expect("send failed");
                sleep(time::Duration::from_micros(1)).await;
                debug!("server: sending 2nd value to client");
                let data2 = vec![0x20, 0x11];
                framed.send(to_bytes(data2)).await.expect("2nd send failed");
            }

            // sleep 1 seconds so we don't lost connection
            sleep(time::Duration::from_secs(1)).await;

            Ok(()) as Result<(), IoError>
        };

        let client_ft = async {
            debug!("client: sleep to give server chance to come up");
            sleep(time::Duration::from_millis(100)).await;
            debug!("client: trying to connect");
            let tcp_stream = TcpStream::connect(&addr).await.expect("connection fail");
            let tls_stream = connector
                .connect("localhost", tcp_stream)
                .await
                .expect("tls failed");
            let all_stream = AllTcpStream::Tls(tls_stream);
            let mut framed = Framed::new(all_stream.compat(), BytesCodec::new());
            debug!("client: got connection. waiting");

            for i in 0..TEST_ITERATION {
                let value = framed.next().await.expect("frame");
                debug!("{} client :received first value from server",i);
                let bytes = value.expect("invalid value");
                let values = bytes.take(3).into_inner();
                assert_eq!(values[0], 0x05);
                assert_eq!(values[1], 0x0a);
                assert_eq!(values[2], 0x63);
                assert_eq!(values.len(), 3);

                let value2 = framed.next().await.expect("frame");
                debug!("client: received 2nd value from server");
                let bytes = value2.expect("packet decoding works");
                let values = bytes.take(2).into_inner();
                assert_eq!(values.len(), 2);
            }

            Ok(()) as Result<(), IoError>
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
