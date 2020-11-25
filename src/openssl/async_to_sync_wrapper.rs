use std::fmt::{Debug, Formatter};
use std::io;
use std::io::Read;
use std::io::Write;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures_lite::io::{AsyncRead, AsyncWrite};

pub(super) struct AsyncToSyncWrapper<S> {
    inner: S,
    context: usize,
}

impl<S> AsyncToSyncWrapper<S>
where
    S: Unpin,
{
    /// Wrap sync object in this wrapper.
    pub fn new(inner: S) -> Self {
        Self { inner, context: 0 }
    }

    /// Store async context inside this object
    pub fn set_context(&mut self, cx: &mut Context<'_>) {
        assert_eq!(self.context, 0);
        self.context = cx as *mut _ as usize;
    }

    /// Clear async context
    pub fn unset_context(&mut self) {
        assert_ne!(self.context, 0);
        self.context = 0;
    }

    fn with_context<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Context<'_>, Pin<&mut S>) -> R,
    {
        unsafe {
            assert_ne!(self.context, 0);
            let waker = &mut *(self.context as *mut _);
            f(waker, Pin::new(&mut self.inner))
        }
    }
}

impl<S> Debug for AsyncToSyncWrapper<S>
where
    S: Debug,
{
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.inner, fmt)
    }
}

impl<S> Read for AsyncToSyncWrapper<S>
where
    S: AsyncRead + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.with_context(|ctx, stream| match stream.poll_read(ctx, buf)? {
            Poll::Ready(num_bytes_read) => Ok(num_bytes_read),
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        })
    }
}

impl<S> Write for AsyncToSyncWrapper<S>
where
    S: AsyncWrite + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.with_context(|ctx, stream| stream.poll_write(ctx, buf)) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.with_context(|ctx, stream| stream.poll_flush(ctx)) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}
