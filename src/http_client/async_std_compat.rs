#![allow(dead_code)] // only for http-client part 1

use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use async_rustls::rustls::{client::InvalidDnsNameError, ClientConfig};
use async_std::io::{Read, Write};
use hyper::{
    client::connect::{Connected, Connection},
    rt,
    service::Service,
    Uri,
};

use crate::{
    net::TcpStream,
    rust_tls::{DefaultClientTlsStream, TlsConnector},
};

const DEFAULT_PORT: u16 = 443;

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("unsupported protocol: {0}")]
    UnsupportedProtocol(String),
    #[error("no host defined: {0:?}")]
    NoHost(Uri),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    InvalidDns(#[from] InvalidDnsNameError),
}

#[derive(Clone)]
pub struct CompatConnector(Arc<TlsConnector>);

impl CompatConnector {
    pub fn new(tls_config: ClientConfig) -> Self {
        Self(Arc::new(TlsConnector::from(Arc::new(tls_config))))
    }
}

impl Service<Uri> for CompatConnector {
    type Response = TlsStream;
    type Error = ConnectorError;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let connector = self.0.clone();

        let fut = async move {
            let host = uri.host().ok_or_else(|| {
                let uri = uri.clone();
                ConnectorError::NoHost(uri)
            })?;

            match uri.scheme_str() {
                Some("https") => {
                    let port = uri.port_u16().unwrap_or(DEFAULT_PORT);
                    let tcp_stream = TcpStream::connect((host, port))
                        .await
                        .map_err(ConnectorError::IoError)?;

                    let stream = connector.connect(host.try_into()?, tcp_stream).await?;
                    Ok(TlsStream(stream))
                }
                Some(scheme) => Err(ConnectorError::UnsupportedProtocol(scheme.to_string())),
                _ => Err(ConnectorError::UnsupportedProtocol(
                    "no scheme provided".to_owned(),
                )),
            }
        };

        Box::pin(fut)
    }
}

pub struct TlsStream(DefaultClientTlsStream);

impl Connection for TlsStream {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl tokio::io::AsyncRead for TlsStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match Pin::new(&mut self.0).poll_read(cx, buf.initialize_unfilled())? {
            Poll::Ready(bytes_read) => {
                buf.advance(bytes_read);
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl tokio::io::AsyncWrite for TlsStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

#[derive(Clone)]
pub struct CompatExecutor;

impl<F> rt::Executor<F> for CompatExecutor
where
    F: Future + Send + 'static,
    F::Output: Send,
{
    fn execute(&self, fut: F) {
        async_std::task::spawn(fut);
    }
}
