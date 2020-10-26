use crate::net::TcpStream;

pub use async_tls::client::TlsStream as ClientTlsStream;
pub use async_tls::server::TlsStream as ServerTlsStream;
pub use async_tls::TlsAcceptor;
pub use async_tls::TlsConnector;

pub type DefaultServerTlsStream = ServerTlsStream<TcpStream>;
pub type DefaultClientTlsStream = ClientTlsStream<TcpStream>;

use rustls::Certificate;
use rustls::ClientConfig;
use rustls::PrivateKey;
use rustls::RootCertStore;
use rustls::ServerConfig;

pub use builder::*;
pub use cert::*;
pub use connector::*;

mod cert {
    use std::fs::File;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::path::Path;

    use rustls::internal::pemfile::certs;
    use rustls::internal::pemfile::rsa_private_keys;

    use super::Certificate;
    use super::PrivateKey;
    use super::RootCertStore;

    pub fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<Certificate>, IoError> {
        load_certs_from_reader(&mut BufReader::new(File::open(path)?))
    }

    pub fn load_certs_from_reader(rd: &mut dyn BufRead) -> Result<Vec<Certificate>, IoError> {
        certs(rd).map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))
    }

    /// Load the passed keys file
    pub fn load_keys<P: AsRef<Path>>(path: P) -> Result<Vec<PrivateKey>, IoError> {
        load_keys_from_reader(&mut BufReader::new(File::open(path)?))
    }

    pub fn load_keys_from_reader(rd: &mut dyn BufRead) -> Result<Vec<PrivateKey>, IoError> {
        rsa_private_keys(rd).map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid key"))
    }

    pub fn load_root_ca<P: AsRef<Path>>(path: P) -> Result<RootCertStore, IoError> {
        let mut root_store = RootCertStore::empty();

        root_store
            .add_pem_file(&mut BufReader::new(File::open(path)?))
            .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid ca crt"))?;

        Ok(root_store)
    }
}

mod connector {

    use std::io::Error as IoError;

    use std::os::unix::io::AsRawFd;
    use std::os::unix::io::RawFd;

    use async_trait::async_trait;

    use crate::net::DefaultTcpDomainConnector;
    use crate::net::TcpDomainConnector;
    use log::debug;

    use super::AllTcpStream;
    use super::DefaultClientTlsStream;
    use super::TcpStream;
    use super::TlsConnector;

    pub type TlsError = IoError;

    /// connect as anonymous client
    #[derive(Clone)]
    pub struct TlsAnonymousConnector(TlsConnector);

    impl From<TlsConnector> for TlsAnonymousConnector {
        fn from(connector: TlsConnector) -> Self {
            Self(connector)
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsAnonymousConnector {
        type WrapperStream = DefaultClientTlsStream;

        async fn connect(&self, domain: &str) -> Result<(Self::WrapperStream, RawFd), IoError> {
            let tcp_stream = TcpStream::connect(domain).await?;
            let fd = tcp_stream.as_raw_fd();
            Ok((self.0.connect(domain, tcp_stream).await?, fd))
        }
    }

    #[derive(Clone)]
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
        type WrapperStream = DefaultClientTlsStream;

        async fn connect(&self, addr: &str) -> Result<(Self::WrapperStream, RawFd), IoError> {
            debug!("connect to tls addr: {}", addr);
            let tcp_stream = TcpStream::connect(addr).await?;
            let fd = tcp_stream.as_raw_fd();

            debug!("connect to tls domain: {}", self.domain);
            Ok((self.connector.connect(&self.domain, tcp_stream).await?, fd))
        }
    }

    #[derive(Clone)]
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

        async fn connect(&self, domain: &str) -> Result<(Self::WrapperStream, RawFd), IoError> {
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

mod builder {

    use std::fs::File;
    use std::io::BufReader;
    use std::io::Cursor;
    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::path::Path;
    use std::sync::Arc;

    use rustls::AllowAnyAuthenticatedClient;
    use rustls::ServerCertVerified;
    use rustls::ServerCertVerifier;
    use rustls::TLSError;
    use webpki::DNSNameRef;

    use super::load_certs;
    use super::load_certs_from_reader;
    use super::load_keys;
    use super::load_keys_from_reader;
    use super::load_root_ca;
    use super::Certificate;
    use super::ClientConfig;
    use super::RootCertStore;
    use super::ServerConfig;
    use super::TlsAcceptor;
    use super::TlsConnector;

    pub struct ConnectorBuilder(ClientConfig);

    impl ConnectorBuilder {
        pub fn new() -> Self {
            Self(ClientConfig::new())
        }

        pub fn load_ca_cert<P: AsRef<Path>>(mut self, path: P) -> Result<Self, IoError> {
            self.0
                .root_store
                .add_pem_file(&mut BufReader::new(File::open(path)?))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid ca crt"))?;

            Ok(self)
        }

        pub fn load_ca_cert_from_bytes(mut self, buffer: &[u8]) -> Result<Self, IoError> {
            let mut bytes = Cursor::new(buffer);
            self.0
                .root_store
                .add_pem_file(&mut bytes)
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid ca crt"))?;

            Ok(self)
        }

        pub fn load_client_certs<P: AsRef<Path>>(
            mut self,
            cert_path: P,
            key_path: P,
        ) -> Result<Self, IoError> {
            let client_certs = load_certs(cert_path)?;
            let mut client_keys = load_keys(key_path)?;
            self.0
                .set_single_client_cert(client_certs, client_keys.remove(0))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))?;

            Ok(self)
        }

        pub fn load_client_certs_from_bytes(
            mut self,
            cert_buf: &[u8],
            key_buf: &[u8],
        ) -> Result<Self, IoError> {
            let client_certs = load_certs_from_reader(&mut Cursor::new(cert_buf))?;
            let mut client_keys = load_keys_from_reader(&mut Cursor::new(key_buf))?;
            self.0
                .set_single_client_cert(client_certs, client_keys.remove(0))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))?;

            Ok(self)
        }

        pub fn no_cert_verification(mut self) -> Self {
            self.0
                .dangerous()
                .set_certificate_verifier(Arc::new(NoCertificateVerification {}));

            self
        }

        pub fn build(self) -> TlsConnector {
            self.0.into()
        }
    }

    pub struct AcceptorBuilder(ServerConfig);

    impl AcceptorBuilder {
        /// create builder with no client authentication
        pub fn new_no_client_authentication() -> Self {
            use rustls::NoClientAuth;

            Self(ServerConfig::new(NoClientAuth::new()))
        }

        /// create builder with client authentication
        /// must pass CA root
        pub fn new_client_authenticate<P: AsRef<Path>>(path: P) -> Result<Self, IoError> {
            let root_store = load_root_ca(path)?;

            Ok(Self(ServerConfig::new(AllowAnyAuthenticatedClient::new(
                root_store,
            ))))
        }

        pub fn load_server_certs<P: AsRef<Path>>(
            mut self,
            cert_path: P,
            key_path: P,
        ) -> Result<Self, IoError> {
            let server_crt = load_certs(cert_path)?;
            let mut server_keys = load_keys(key_path)?;
            self.0
                .set_single_cert(server_crt, server_keys.remove(0))
                .map_err(|_| IoError::new(ErrorKind::InvalidInput, "invalid cert"))?;

            Ok(self)
        }

        pub fn build(self) -> TlsAcceptor {
            TlsAcceptor::from(Arc::new(self.0))
        }
    }

    struct NoCertificateVerification {}

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _roots: &RootCertStore,
            _presented_certs: &[Certificate],
            _dns_name: DNSNameRef<'_>,
            _ocsp: &[u8],
        ) -> Result<ServerCertVerified, TLSError> {
            log::debug!("ignoring server cert");
            Ok(ServerCertVerified::assertion())
        }
    }
}

pub use stream::AllTcpStream;

mod stream {

    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::DefaultClientTlsStream;
    use super::TcpStream;
    use futures_lite::{AsyncRead, AsyncWrite};
    use pin_project::pin_project;

    #[pin_project(project = EnumProj)]
    pub enum AllTcpStream {
        Tcp(#[pin] TcpStream),
        Tls(#[pin] DefaultClientTlsStream),
    }

    impl AllTcpStream {
        pub fn tcp(stream: TcpStream) -> Self {
            Self::Tcp(stream)
        }

        pub fn tls(stream: DefaultClientTlsStream) -> Self {
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
