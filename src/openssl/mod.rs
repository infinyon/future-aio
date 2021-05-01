mod acceptor;
mod async_to_sync_wrapper;
mod certificate;
mod connector;
mod error;
mod handshake;
mod stream;
#[cfg(test)]
mod test;

pub use acceptor::{TlsAcceptor, TlsAcceptorBuilder};
pub use certificate::Certificate;
pub use connector::{TlsAnonymousConnector, TlsConnector, TlsConnectorBuilder, TlsDomainConnector};
pub use error::Error as TlsError;
pub use openssl::ssl::SslVerifyMode;
pub use stream::{AllTcpStream, TlsStream};

pub type DefaultServerTlsStream = TlsStream<crate::net::TcpStream>;
pub type DefaultClientTlsStream = TlsStream<crate::net::TcpStream>;

mod split {

    use async_net::TcpStream;
    use futures_util::AsyncReadExt;

    use super::*;
    use crate::net::{BoxReadConnection, BoxWriteConnection, SplitConnection};

    impl SplitConnection for TlsStream<TcpStream> {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            let (read, write) = self.split();
            (Box::new(write), Box::new(read))
        }
    }
}
