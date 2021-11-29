use std::fmt::Debug;

use thiserror::Error;

use super::async_to_sync_wrapper::AsyncToSyncWrapper;

#[derive(Error, Debug)]
pub enum Error {
    #[error("OpenSslError: {0}")]
    OpenSslError(#[from] openssl::error::Error),

    #[error("HandshakeError: {0}")]
    HandshakeError(Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("ErrorStack: {0}")]
    ErrorStack(#[from] openssl::error::ErrorStack),

    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),
}

impl<S: Debug + Send + Sync + 'static> From<openssl::ssl::HandshakeError<AsyncToSyncWrapper<S>>>
    for Error
{
    fn from(handshake_error: openssl::ssl::HandshakeError<AsyncToSyncWrapper<S>>) -> Self {
        Self::HandshakeError(Box::new(handshake_error))
    }
}

impl Error {
    pub fn into_io_error(self) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, self)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
