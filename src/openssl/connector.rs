use std::fmt;
use std::io;
use std::path::Path;

use async_trait::async_trait;
use futures_lite::io::{AsyncRead, AsyncWrite};
use log::debug;
use openssl::ssl;
use openssl::x509::verify::X509VerifyFlags;

use crate::net::{
    tcp_stream::{stream, stream_with_opts, SocketOpts},
    AsConnectionFd, BoxReadConnection, BoxWriteConnection, ConnectionFd, DomainConnector,
    SplitConnection, TcpDomainConnector,
};

use super::async_to_sync_wrapper::AsyncToSyncWrapper;
use super::certificate::Certificate;
use super::error::Result;
use super::handshake::HandshakeFuture;
use super::stream::TlsStream;

// same code but without native builder
// TODO: simplification
pub mod certs {

    use std::io::Error as IoError;
    use std::io::ErrorKind;

    use openssl::pkcs12::Pkcs12;
    use openssl::pkey::Private;

    use super::Certificate;
    use crate::net::certs::CertBuilder;

    pub type PrivateKey = openssl::pkey::PKey<Private>;

    // use identity_impl::Certificate;
    use identity_impl::Identity;

    // copied from https://github.com/sfackler/rust-native-tls/blob/master/src/imp/openssl.rs
    mod identity_impl {

        use crate::openssl::TlsError::CertReadError;
        use openssl::pkcs12::Pkcs12;
        use openssl::pkey::{PKey, Private};
        use openssl::x509::X509;

        #[derive(Clone)]
        pub struct Identity {
            pkey: PKey<Private>,
            cert: X509,
            chain: Vec<X509>,
        }

        impl Identity {
            pub fn from_pkcs12(buf: &[u8], pass: &str) -> anyhow::Result<Identity> {
                let pkcs12 = Pkcs12::from_der(buf)?;
                let parsed = pkcs12.parse2(pass)
                    .map_err(|_| {CertReadError(String::from("Couldn't read pkcs12"))})?;
                let pkey = parsed.pkey.ok_or(CertReadError(String::from("Missing private key")))?;
                let cert = parsed.cert.ok_or(CertReadError(String::from("Missing cert")))?;
                Ok(Identity {
                    pkey,
                    cert,
                    chain: parsed.ca.into_iter().flatten().collect(),
                })
            }

            pub fn cert(&self) -> &X509 {
                &self.cert
            }

            pub fn pkey(&self) -> &PKey<Private> {
                &self.pkey
            }

            pub fn chain(&self) -> &Vec<X509> {
                &self.chain
            }
        }
    }

    pub struct X509PemBuilder(Vec<u8>);

    impl CertBuilder for X509PemBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl X509PemBuilder {
        pub fn build(self) -> Result<Certificate, IoError> {
            let cert = Certificate::from_pem(&self.0).map_err(|err| {
                IoError::new(ErrorKind::InvalidInput, format!("invalid cert: {}", err))
            })?;
            Ok(cert)
        }
    }

    const PASSWORD: &str = "test";

    pub struct PrivateKeyBuilder(Vec<u8>);

    impl CertBuilder for PrivateKeyBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl PrivateKeyBuilder {
        pub fn build(self) -> Result<PrivateKey, IoError> {
            let key = PrivateKey::private_key_from_pem(&self.0).map_err(|err| {
                IoError::new(ErrorKind::InvalidInput, format!("invalid key: {}", err))
            })?;
            Ok(key)
        }
    }

    pub struct IdentityBuilder(Vec<u8>);

    impl CertBuilder for IdentityBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl IdentityBuilder {
        /// load pk12 from x509 certs
        pub fn from_x509(x509: X509PemBuilder, key: PrivateKeyBuilder) -> Result<Self, IoError> {
            let server_key = key.build()?;
            let server_crt = x509.build()?;
            let p12 = Pkcs12::builder()
                .name("")
                .pkey(&server_key)
                .cert(server_crt.inner())
                .build2(PASSWORD)
                .map_err(|e| {
                    IoError::new(
                        ErrorKind::InvalidData,
                        format!("Failed to create Pkcs12: {}", e),
                    )
                })?;

            let der = p12.to_der()?;
            Ok(Self(der))
        }

        pub fn build(self) -> Result<Identity, IoError> {
            Identity::from_pkcs12(&self.0, PASSWORD).map_err(|e| {
                IoError::new(ErrorKind::InvalidData, format!("Failed to load der: {}", e))
            })
        }
    }
}

#[derive(Clone, Debug)]
pub struct TlsConnector {
    pub inner: ssl::SslConnector,
    pub verify_hostname: bool,
    pub allow_partial: bool,
}

impl TlsConnector {
    pub fn builder() -> Result<TlsConnectorBuilder> {
        let inner = ssl::SslConnector::builder(ssl::SslMethod::tls())?;
        Ok(TlsConnectorBuilder {
            inner,
            verify_hostname: true,
            allow_partial: true,
        })
    }

    pub async fn connect<S>(&self, domain: &str, stream: S) -> Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + fmt::Debug + Unpin + Send + Sync + 'static,
    {
        debug!("tls connecting to: {}", domain);
        let mut client_configuration = self
            .inner
            .configure()?
            .verify_hostname(self.verify_hostname);

        if self.allow_partial {
            let params = client_configuration.param_mut();
            params.set_flags(X509VerifyFlags::PARTIAL_CHAIN)?;
        }

        HandshakeFuture::Initial(
            move |stream| client_configuration.connect(domain, stream),
            AsyncToSyncWrapper::new(stream),
        )
        .await
    }
}

pub struct TlsConnectorBuilder {
    inner: ssl::SslConnectorBuilder,
    verify_hostname: bool,
    allow_partial: bool,
}

impl TlsConnectorBuilder {
    pub fn with_hostname_verification_disabled(mut self) -> Result<TlsConnectorBuilder> {
        self.verify_hostname = false;
        Ok(self)
    }

    pub fn with_certificate_verification_disabled(mut self) -> Result<TlsConnectorBuilder> {
        self.inner.set_verify(ssl::SslVerifyMode::NONE);
        Ok(self)
    }

    pub fn with_certifiate_and_key_from_pem_files<P: AsRef<Path>>(
        mut self,
        cert_file: P,
        key_file: P,
    ) -> Result<TlsConnectorBuilder> {
        self.inner
            .set_certificate_file(cert_file, ssl::SslFiletype::PEM)?;
        self.inner
            .set_private_key_file(key_file, ssl::SslFiletype::PEM)?;
        Ok(self)
    }

    pub fn with_ca_from_pem_file<P: AsRef<Path>>(
        mut self,
        ca_file: P,
    ) -> Result<TlsConnectorBuilder> {
        self.inner.set_ca_file(ca_file)?;
        Ok(self)
    }

    pub fn add_root_certificate(mut self, cert: Certificate) -> Result<TlsConnectorBuilder> {
        self.inner.cert_store_mut().add_cert(cert.0)?;
        Ok(self)
    }

    /// set identity
    pub fn with_identity(mut self, builder: certs::IdentityBuilder) -> Result<Self> {
        let identity = builder.build()?;
        self.inner.set_certificate(identity.cert())?;
        self.inner.set_private_key(identity.pkey())?;
        for cert in identity.chain().iter().rev() {
            self.inner.add_extra_chain_cert(cert.to_owned())?;
        }
        Ok(self)
    }

    pub fn build(self) -> TlsConnector {
        TlsConnector {
            inner: self.inner.build(),
            verify_hostname: self.verify_hostname,
            allow_partial: self.allow_partial,
        }
    }
}

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
    async fn connect(
        &self,
        domain: &str,
    ) -> io::Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd)> {
        debug!("tcp connect: {}", domain);
        let socket_opts = SocketOpts {
            keepalive: Some(Default::default()),
            nodelay: Some(true),
            ..Default::default()
        };
        let tcp_stream = stream_with_opts(domain, Some(socket_opts)).await?;
        let fd = tcp_stream.as_connection_fd();

        let (write, read) = self
            .0
            .connect(domain, tcp_stream)
            .await
            .map_err(|err| err.into_io_error())?
            .split_connection();

        Ok((write, read, fd))
    }

    fn new_domain(&self, _domain: String) -> DomainConnector {
        Box::new(self.clone())
    }

    fn domain(&self) -> &str {
        "localhost"
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
    async fn connect(
        &self,
        addr: &str,
    ) -> io::Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd)> {
        debug!("connect to tls addr: {}", addr);
        let tcp_stream = stream(addr).await?;
        let fd = tcp_stream.as_connection_fd();

        let (write, read) = self
            .connector
            .connect(&self.domain, tcp_stream)
            .await
            .map_err(|err| err.into_io_error())?
            .split_connection();

        debug!("connect to tls domain: {}", self.domain);
        Ok((write, read, fd))
    }

    fn new_domain(&self, domain: String) -> DomainConnector {
        debug!("setting new domain: {}", domain);
        let mut connector = self.clone();
        connector.domain = domain;
        Box::new(connector)
    }

    fn domain(&self) -> &str {
        &self.domain
    }
}
