use std::fmt;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::result;
use std::task::Context;
use std::task::Poll;

use anyhow::{Error, Result};
use futures_lite::io::{AsyncRead, AsyncWrite};

use super::async_to_sync_wrapper::AsyncToSyncWrapper;
use super::stream::TlsStream;

pub(super) enum HandshakeFuture<F, S: Unpin + fmt::Debug> {
    Initial(F, AsyncToSyncWrapper<S>),
    MidHandshake(openssl::ssl::MidHandshakeSslStream<AsyncToSyncWrapper<S>>),
    Done,
}

impl<F, S> Future for HandshakeFuture<F, S>
where
    S: AsyncRead + AsyncWrite + fmt::Debug + Unpin + Sync + Send + 'static,
    F: FnOnce(
        AsyncToSyncWrapper<S>,
    ) -> result::Result<
        openssl::ssl::SslStream<AsyncToSyncWrapper<S>>,
        openssl::ssl::HandshakeError<AsyncToSyncWrapper<S>>,
    >,
    Self: Unpin,
{
    type Output = Result<TlsStream<S>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let self_mut = self.get_mut();
        match mem::replace(self_mut, HandshakeFuture::Done) {
            HandshakeFuture::Initial(f, mut stream) => {
                stream.set_context(cx);

                match f(stream) {
                    Ok(mut stream) => {
                        stream.get_mut().unset_context();
                        Poll::Ready(Ok(TlsStream(stream)))
                    }
                    Err(openssl::ssl::HandshakeError::WouldBlock(mut mid)) => {
                        mid.get_mut().unset_context();
                        *self_mut = HandshakeFuture::MidHandshake(mid);
                        Poll::Pending
                    }
                    Err(e) => Poll::Ready(Err(Error::new(e))),
                }
            }
            HandshakeFuture::MidHandshake(mut stream) => {
                stream.get_mut().set_context(cx);
                match stream.handshake() {
                    Ok(mut stream) => {
                        stream.get_mut().unset_context();
                        Poll::Ready(Ok(TlsStream(stream)))
                    }
                    Err(openssl::ssl::HandshakeError::WouldBlock(mut mid)) => {
                        mid.get_mut().unset_context();
                        *self_mut = HandshakeFuture::MidHandshake(mid);
                        Poll::Pending
                    }
                    Err(e) => Poll::Ready(Err(Error::new(e))),
                }
            }
            HandshakeFuture::Done => panic!("Future must not be polled after ready"),
        }
    }
}
