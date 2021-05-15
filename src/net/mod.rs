#[cfg(unix)]
pub use async_net::*;

#[cfg(test)]
#[cfg(unix)]
mod tcp_stream;

#[cfg(unix)]
pub use connector::*;

pub use conn::*;

mod conn {

    use futures_lite::io::{AsyncRead, AsyncWrite};

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
}

#[cfg(unix)]
mod connector {
    use super::TcpStream;
    use std::io::Error as IoError;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::io::RawFd;
    impl SplitConnection for TcpStream {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            (Box::new(self.clone()), Box::new(self))
        }
    }

    use async_trait::async_trait;
    use log::debug;

    use super::*;

    pub type DomainConnector = Box<dyn TcpDomainConnector>;

    /// connect to domain and return connection
    #[async_trait]
    pub trait TcpDomainConnector: Send + Sync {
        async fn connect(
            &self,
            domain: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, RawFd), IoError>;

        // create new version of my self with new domain
        fn new_domain(&self, domain: String) -> DomainConnector;

        fn domain(&self) -> &str;
    }

    /// creatges TcpStream connection
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
        ) -> Result<(BoxWriteConnection, BoxReadConnection, RawFd), IoError> {
            debug!("connect to tcp addr: {}", addr);
            let tcp_stream = TcpStream::connect(addr).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((Box::new(tcp_stream.clone()), Box::new(tcp_stream), fd))
        }

        fn new_domain(&self, _domain: String) -> DomainConnector {
            Box::new(self.clone())
        }

        fn domain(&self) -> &str {
            "localhost"
        }
    }
}

#[cfg(test)]
mod test {
    use std::time;

    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_util::AsyncReadExt;
    use log::debug;

    use crate::net::TcpListener;
    use crate::net::TcpStream;
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
            let tcp_stream = TcpStream::connect(&addr).await.expect("test");
            let (_read, _write) = tcp_stream.split();
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
