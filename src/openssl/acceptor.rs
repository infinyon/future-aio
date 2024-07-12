use std::fmt;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use futures_lite::io::{AsyncRead, AsyncWrite};
use openssl::ssl;

use super::async_to_sync_wrapper::AsyncToSyncWrapper;
use super::certificate::{Certificate, PrivateKey};
use super::handshake::HandshakeFuture;
use super::stream::TlsStream;

#[derive(Clone)]
pub struct TlsAcceptor(pub Arc<ssl::SslAcceptor>);

impl TlsAcceptor {
    pub fn builder() -> Result<TlsAcceptorBuilder> {
        let inner =
            openssl::ssl::SslAcceptor::mozilla_intermediate_v5(openssl::ssl::SslMethod::tls())?;

        Ok(TlsAcceptorBuilder { inner })
    }

    pub async fn accept<S>(&self, stream: S) -> Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send + Sync + 'static,
    {
        HandshakeFuture::Initial(
            move |stream| self.0.accept(stream),
            AsyncToSyncWrapper::new(stream),
        )
        .await
    }
}

pub struct TlsAcceptorBuilder {
    pub inner: ssl::SslAcceptorBuilder,
}

impl TlsAcceptorBuilder {
    pub fn with_certifiate_and_key_from_pem_files<P: AsRef<Path>>(
        mut self,
        cert_file: P,
        key_file: P,
    ) -> Result<TlsAcceptorBuilder> {
        self.inner
            .set_certificate_file(cert_file, ssl::SslFiletype::PEM)?;
        self.inner
            .set_private_key_file(key_file, ssl::SslFiletype::PEM)?;
        Ok(self)
    }

    pub fn with_certifiate_and_key(
        mut self,
        cert: Certificate,
        key: PrivateKey,
    ) -> Result<TlsAcceptorBuilder> {
        self.inner.set_certificate(&cert.0)?;
        self.inner.set_private_key(&key.0)?;
        Ok(self)
    }

    pub fn with_chain(mut self, chain: Vec<Certificate>) -> Result<TlsAcceptorBuilder> {
        for cert in chain {
            self.inner.add_extra_chain_cert(cert.0)?;
        }
        Ok(self)
    }

    pub fn with_ca_from_pem_file<P: AsRef<Path>>(
        mut self,
        ca_file: P,
    ) -> Result<TlsAcceptorBuilder> {
        self.inner.set_ca_file(ca_file)?;
        Ok(self)
    }

    pub fn with_ssl_verify_mode(mut self, mode: ssl::SslVerifyMode) -> TlsAcceptorBuilder {
        self.inner.set_verify(mode);
        self
    }

    pub fn build(self) -> TlsAcceptor {
        TlsAcceptor(Arc::new(self.inner.build()))
    }
}
