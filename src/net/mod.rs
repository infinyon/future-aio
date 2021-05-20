#[cfg(unix)]
pub use async_net::*;

#[cfg(test)]
#[cfg(unix)]
mod tcp_stream;

pub use conn::*;

#[cfg(unix)]
pub use unix_connector::DefaultTcpDomainConnector as DefaultDomainConnector;

#[cfg(target_arch = "wasm32")]
pub use wasm_connector::DefaultDomainWebsocketConnector as DefaultDomainConnector;

mod conn {

    use std::io::Error as IoError;

    use async_trait::async_trait;
    use futures_lite::io::{AsyncRead, AsyncWrite};

    cfg_if::cfg_if! {
        if #[cfg(not(target_arch = "wasm32"))] {
            pub trait Connection: AsyncRead + AsyncWrite + Send + Sync + Unpin + SplitConnection {}
            impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin + SplitConnection> Connection for T {}

            pub trait ReadConnection: AsyncRead + Send + Sync + Unpin {}
            impl<T: AsyncRead + Send + Sync + Unpin> ReadConnection for T {}

            pub trait WriteConnection: AsyncWrite + Send + Sync + Unpin {}
            impl<T: AsyncWrite + Send + Sync + Unpin> WriteConnection for T {}
        } else if #[cfg(target_arch = "wasm32")] {
            pub trait Connection: AsyncRead + AsyncWrite + Unpin + SplitConnection {}
            impl<T: AsyncRead + AsyncWrite  + Unpin + SplitConnection> Connection for T {}

            pub trait ReadConnection: AsyncRead + Unpin {}
            impl<T: AsyncRead + Unpin> ReadConnection for T {}

            pub trait WriteConnection: AsyncWrite + Unpin {}
            impl<T: AsyncWrite + Unpin> WriteConnection for T {}
        }
    }
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
        } else if #[cfg(windows)] {
            pub type ConnectionFd = std::os::windows::raw::SOCKET;
        } else {
            pub type ConnectionFd = String;
        }
    }

    pub type DomainConnector = Box<dyn TcpDomainConnector>;

    /// connect to domain and return connection
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    pub trait TcpDomainConnector {
        async fn connect(
            &self,
            domain: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError>;

        // create new version of my self with new domain
        fn new_domain(&self, domain: String) -> DomainConnector;

        fn domain(&self) -> &str;
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_connector {
    use super::*;
    use async_trait::async_trait;
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
            let (mut _ws, wsstream) = WsMeta::connect(addr, None).await.unwrap();
            // wsstream implements both AsyncRead and AsyncWrite but there's not a good way to
            // split them.
            let _wsstream = wsstream.into_io();
            unimplemented!();
        }

        fn new_domain(&self, _domain: String) -> DomainConnector {
            Box::new(self.clone())
        }

        fn domain(&self) -> &str {
            "localhost"
        }
    }
}

#[cfg(unix)]
mod unix_connector {
    use std::io::Error as IoError;
    use std::os::unix::io::AsRawFd;

    use async_trait::async_trait;
    use log::debug;

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
#[cfg(unix)]
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
