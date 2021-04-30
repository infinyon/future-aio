pub use async_net::*;

#[cfg(test)]
mod tcp_stream;

#[cfg(unix)]
pub use connector::*;

pub use conn::*;

mod conn {

    use futures_lite::io::{AsyncRead, AsyncWrite};

    pub trait Connection: AsyncRead + AsyncWrite + Send + Sync + Unpin {}
    impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin> Connection for T {}

    pub type BoxConnection = Box<dyn Connection>;

    trait ConnectionClone {
        fn clone_box(&self) -> BoxConnection;
    }

    impl<T> ConnectionClone for T
    where
        T: 'static + Connection + Clone,
    {
        fn clone_box(&self) -> BoxConnection {
            Box::new(self.clone())
        }
    }

    // trick from: https://chaoslibrary.blot.im/rust-cloning-a-trait-object
    impl Clone for BoxConnection {
        fn clone(&self) -> BoxConnection {
            self.clone_box()
        }
    }
}

#[cfg(unix)]
mod connector {
    use std::io::Error as IoError;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::io::RawFd;

    use async_trait::async_trait;
    use log::debug;

    use super::*;

    pub type DomainConnector = Box<dyn TcpDomainConnector>;

    /// connect to domain and return connection
    #[async_trait]
    pub trait TcpDomainConnector: Send + Sync {
        async fn connect(&self, domain: &str) -> Result<(BoxConnection, RawFd), IoError>;
    }

    trait DomainConnectorClone {
        fn clone_box(&self) -> DomainConnector;
    }

    impl<T> DomainConnectorClone for T
    where
        T: 'static + TcpDomainConnector + Clone,
    {
        fn clone_box(&self) -> DomainConnector {
            Box::new(self.clone())
        }
    }

    /// creatges TcpStream connection
    #[derive(Clone)]
    pub struct DefaultTcpDomainConnector {}

    impl DefaultTcpDomainConnector {
        pub fn new() -> Self {
            Self {}
        }
    }

    #[async_trait]
    impl TcpDomainConnector for DefaultTcpDomainConnector {
        async fn connect(&self, addr: &str) -> Result<(BoxConnection, RawFd), IoError> {
            debug!("connect to tcp addr: {}", addr);
            let tcp_stream = TcpStream::connect(addr).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((Box::new(tcp_stream), fd))
        }
    }
}
