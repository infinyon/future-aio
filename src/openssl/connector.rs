use std::fmt;
use std::io;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::RawFd;
use std::path::Path;

use async_trait::async_trait;
use futures_lite::io::{AsyncRead, AsyncWrite};
use log::debug;
use openssl::ssl;

use crate::net::{
    BoxReadConnection, BoxWriteConnection, DomainConnector, SplitConnection, TcpDomainConnector,
    TcpStream,
};

use super::async_to_sync_wrapper::AsyncToSyncWrapper;
use super::certificate::Certificate;
use super::error::Result;
use super::handshake::HandshakeFuture;
use super::stream::TlsStream;

#[derive(Clone, Debug)]
pub struct TlsConnector {
    pub inner: ssl::SslConnector,
    pub verify_hostname: bool,
}

impl TlsConnector {
    pub fn builder() -> Result<TlsConnectorBuilder> {
        let inner = ssl::SslConnector::builder(ssl::SslMethod::tls())?;
        Ok(TlsConnectorBuilder {
            inner,
            verify_hostname: true,
        })
    }

    pub async fn connect<S>(&self, domain: &str, stream: S) -> Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + fmt::Debug + Unpin + Send + Sync + 'static,
    {
        let client_configuration = self
            .inner
            .configure()?
            .verify_hostname(self.verify_hostname);
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
}

impl TlsConnectorBuilder {
    pub fn with_hostname_vertification_disabled(mut self) -> Result<TlsConnectorBuilder> {
        self.verify_hostname = false;
        Ok(self)
    }

    pub fn with_certificate_vertification_disabled(mut self) -> Result<TlsConnectorBuilder> {
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

    pub fn build(self) -> TlsConnector {
        TlsConnector {
            inner: self.inner.build(),
            verify_hostname: self.verify_hostname,
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
    ) -> io::Result<(BoxWriteConnection, BoxReadConnection, RawFd)> {
        let tcp_stream = TcpStream::connect(domain).await?;
        let fd = tcp_stream.as_raw_fd();
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
    ) -> io::Result<(BoxWriteConnection, BoxReadConnection, RawFd)> {
        debug!("connect to tls addr: {}", addr);
        let tcp_stream = TcpStream::connect(addr).await?;
        let fd = tcp_stream.as_raw_fd();

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
        let mut connector = self.clone();
        connector.domain = domain;
        Box::new(connector)
    }

    fn domain(&self) -> &str {
        "localhost"
    }
}
