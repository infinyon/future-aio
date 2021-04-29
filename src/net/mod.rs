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
}

#[cfg(unix)]
mod connector {
    use std::io::Error as IoError;
    use std::os::unix::io::AsRawFd;
    use std::os::unix::io::RawFd;

    use async_trait::async_trait;
    use log::debug;

    use super::*;

    /// connect to domain and return connection
    #[async_trait]
    pub trait TcpDomainConnector {
        async fn connect(&self, domain: &str) -> Result<(BoxConnection, RawFd), IoError>;
    }

    /// creatges TcpStream connection
    #[derive(Clone)]
    pub struct DefaultTcpDomainConnector;

    impl DefaultTcpDomainConnector {
        #[allow(clippy::new_without_default)]
        #[deprecated(since = "0.1.13", note = "Please use directly as ZST")]
        pub fn new() -> Self {
            Self
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
