use std::fmt::Debug;
use std::io;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_lite::{AsyncRead, AsyncWrite};
use openssl::ssl;

use super::async_to_sync_wrapper::AsyncToSyncWrapper;
use super::certificate::Certificate;

#[derive(Debug)]
pub struct TlsStream<S>(pub(super) ssl::SslStream<AsyncToSyncWrapper<S>>);

impl<S: Unpin> TlsStream<S> {
    pub fn peer_certificate(&self) -> Option<Certificate> {
        self.0.ssl().peer_certificate().map(Certificate)
    }

    fn with_context<F, R>(&mut self, cx: &mut Context<'_>, f: F) -> Poll<io::Result<R>>
    where
        F: FnOnce(&mut ssl::SslStream<AsyncToSyncWrapper<S>>) -> io::Result<R>,
    {
        self.0.get_mut().set_context(cx);
        let r = f(&mut self.0);
        self.0.get_mut().unset_context();
        result_to_poll(r)
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncRead for TlsStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        self.with_context(cx, |stream| stream.read(buf))
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> AsyncWrite for TlsStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.with_context(cx, |stream| stream.write(buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.with_context(cx, |stream| stream.flush())
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.with_context(cx, |stream| match stream.shutdown() {
            Ok(_) => Ok(()),
            Err(ref e) if e.code() == openssl::ssl::ErrorCode::ZERO_RETURN => Ok(()),
            Err(e) => Err(io::Error::other(e)),
        })
    }
}

fn result_to_poll<T>(r: io::Result<T>) -> Poll<io::Result<T>> {
    match r {
        Ok(v) => Poll::Ready(Ok(v)),
        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
        Err(e) => Poll::Ready(Err(e)),
    }
}
