
#[cfg(feature = "asyncstd")]
pub use async_std::net::*;


#[cfg(feature = "tokio2")]
pub use tokio::net::*;



#[cfg(test)]
mod tcp_stream;


#[cfg(feature = "tls")]
#[cfg(unix)]
pub mod tls;

#[cfg(unix)]
pub use connector::*;

#[cfg(unix)]
mod connector {
    use std::io::Error as IoError;
    #[cfg(unix)]
    use std::os::unix::io::RawFd;
    #[cfg(unix)]
    use std::os::unix::io::AsRawFd;

    use log::debug;
    use futures::io::{AsyncRead, AsyncWrite};
    use async_trait::async_trait;

    use super::TcpStream;

    /// transform raw tcp stream to another stream
    #[async_trait]
    pub trait TcpDomainConnector {

        type WrapperStream: AsyncRead + AsyncWrite + Unpin + Send;

        async fn connect(&self,domain: &str) -> Result<(Self::WrapperStream,RawFd),IoError>;
    }


    #[derive(Clone)]
    pub struct DefaultTcpDomainConnector{}

    impl DefaultTcpDomainConnector {
        pub fn new() -> Self {
            Self{}
        }
    }

    #[async_trait]
    impl TcpDomainConnector for DefaultTcpDomainConnector {

        type WrapperStream = TcpStream;

        async fn connect(&self,domain: &str) -> Result<(Self::WrapperStream,RawFd),IoError> {
            debug!("connect to tcp addr: {}",domain);
            let tcp_stream = TcpStream::connect(domain).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((tcp_stream,fd))
        }
    }


}


