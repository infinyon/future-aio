use std::io::Error as IoError;
use std::os::unix::io::AsRawFd;
use thiserror::Error;

use async_trait::async_trait;
#[allow(unused)]
use nix::libc::off_t;
use nix::sys::sendfile::sendfile;
use nix::Error as NixError;

use log::debug;
use log::trace;

use crate::task::spawn_blocking;

use crate::file_slice::AsyncFileSlice;

#[derive(Error, Debug)]
pub enum SendFileError {
    #[error("IO error: {source}")]
    IoError {
        #[from]
        source: IoError,
    },
    #[error("Nix error: {source}")]
    NixError {
        #[from]
        source: NixError,
    },
}

/// zero copy write
#[async_trait]
pub trait ZeroCopyWrite {
    async fn zero_copy_write(&mut self, source: &AsyncFileSlice) -> Result<usize, SendFileError>;
}

#[async_trait]
impl<T> ZeroCopyWrite for T
where
    T: AsRawFd + Send,
{
    async fn zero_copy_write(&mut self, source: &AsyncFileSlice) -> Result<usize, SendFileError> {
        let size = source.len();
        let target_fd = self.as_raw_fd();
        let source_fd = source.fd();

        #[cfg(target_os = "linux")]
        let ft = {
            let offset = source.position() as off_t;

            spawn_blocking(move || {
                let mut total_transferred: usize = 0; // total bytes transferred so far
                let mut current_offset = offset;

                loop {
                    let to_be_transfer = size as usize - total_transferred;

                    trace!(
                        "trying: zero copy source fd: {} offset: {} len: {}, target: fd{}",
                        source_fd,
                        current_offset,
                        to_be_transfer,
                        target_fd
                    );

                    match sendfile(
                        target_fd,
                        source_fd,
                        Some(&mut current_offset),
                        to_be_transfer,
                    ) {
                        Ok(len) => {
                            total_transferred += len as usize;
                            current_offset += len as off_t;
                            trace!(
                                "actual: zero copy bytes transferred: {} out of {}",
                                len,
                                size
                            );

                            if total_transferred < size as usize {
                                debug!(
                                    "current transferred: {} less than total: {}, continuing",
                                    total_transferred, size
                                );
                            } else {
                                return Ok(len as usize);
                            }
                        }
                        Err(err) => {
                            log::error!("error sendfile: {}", err);
                            return Err(err.into());
                        }
                    }
                }
            })
        };

        #[cfg(target_os = "macos")]
        let ft = {
            let offset = source.position();
            spawn_blocking(move || {
                use nix::errno::Errno;

                let mut total_transferred = 0;
                let mut current_offset = offset as u64;

                loop {
                    let to_be_transfer = (size - total_transferred) as i64;

                    trace!(
                        "mac zero copy source fd: {} offset: {} len: {}, target: fd{}",
                        source_fd,
                        current_offset,
                        to_be_transfer,
                        target_fd
                    );

                    let (res, len) = sendfile(
                        source_fd,
                        target_fd,
                        current_offset as i64,
                        Some(to_be_transfer),
                        None,
                        None,
                    );

                    trace!("mac zero copy bytes transferred: {}", len);
                    total_transferred += len as u64;
                    current_offset += len as u64;
                    match res {
                        Ok(_) => {
                            if total_transferred < size {
                                debug!(
                                    "current transferred: {} less than total: {}, continuing",
                                    total_transferred, size
                                );
                            } else {
                                return Ok(len as usize);
                            }
                        }
                        Err(err) => {
                            if let NixError::Sys(err_no) = err {
                                if err_no == Errno::EAGAIN {
                                    debug!("EAGAIN, try again");
                                    continue;
                                }
                            }

                            log::error!("error sendfile: {}", err);
                            return Err(err.into());
                        }
                    }
                }
            })
        };

        ft.await
    }
}

#[cfg(test)]
mod tests {

    use std::net::SocketAddr;
    use std::time;

    use futures_lite::future::zip;
    use futures_util::stream::StreamExt;
    use log::debug;

    use crate::fs::util as file_util;
    use crate::fs::AsyncFileExtension;
    use crate::net::TcpListener;
    use crate::net::TcpStream;
    use crate::test_async;
    use crate::timer::sleep;
    use crate::zero_copy::ZeroCopyWrite;
    use futures_lite::AsyncReadExt;

    use super::SendFileError;

    const CONST_TEST_ADDR: &str = "127.0.0.1:9999";
    const ZERO_COPY_PORT: u16 = 8888;

    #[test_async]
    async fn test_zero_copy_from_fs_to_socket() -> Result<(), SendFileError> {
        // spawn tcp client and check contents
        let server = async {
            #[allow(unused_mut)]
            let mut listener = TcpListener::bind(CONST_TEST_ADDR).await?;

            debug!("server: listening");
            let mut incoming = listener.incoming();
            if let Some(stream) = incoming.next().await {
                debug!("server: got connection. waiting");
                let mut tcp_stream = stream?;
                let mut buf = [0; 30];
                let len = tcp_stream.read(&mut buf).await?;
                assert_eq!(len, 30);
            } else {
                panic!("client should connect");
            }
            Ok(()) as Result<(), SendFileError>
        };

        let client = async {
            let file = file_util::open("test-data/apirequest.bin").await?;
            sleep(time::Duration::from_millis(100)).await;
            let addr = CONST_TEST_ADDR.parse::<SocketAddr>().expect("parse");
            debug!("client: file loaded");
            let mut stream = TcpStream::connect(&addr).await?;
            debug!("client: connected to server");
            let f_slice = file.as_slice(0, None).await?;
            debug!("client: send back file using zero copy");
            stream.zero_copy_write(&f_slice).await?;
            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream

        let _ = zip(client, server).await;
        Ok(())
    }

    #[test_async]
    async fn test_zero_copy_large_size() -> Result<(), SendFileError> {
        const MAX_BYTES: usize = 300000;

        use futures_lite::AsyncWriteExt;
        use std::env::temp_dir;

        const TEST_ITERATION: u16 = 20;

        async fn init_file() {
            let temp_file = temp_dir().join("async_large");
            debug!("temp file: {:#?}", temp_file);
            let mut file = file_util::create(temp_file.clone())
                .await
                .expect("file creation");

            let bytes: Vec<u8> = vec![0; 1000];
            for _ in 0..300 {
                file.write_all(&bytes).await.expect("writing");
            }

            file.sync_all().await.expect("flushing");
            drop(file);
            debug!("finish creating large test file");
        }

        // spawn tcp client and check contents
        let server = async {
            init_file().await;

            let temp_file = temp_dir().join("async_large");
            // do zero copy
            let file = file_util::open(&temp_file).await.expect("re opening");
            let f_slice = file.as_slice(0, None).await.expect("filed opening");
            assert_eq!(f_slice.len(), MAX_BYTES as u64);

            let listener = TcpListener::bind(format!("127.0.0.1:{}", ZERO_COPY_PORT))
                .await
                .expect("failed bind");

            debug!("large server: listening");
            let mut incoming = listener.incoming();

            for i in 0..TEST_ITERATION {
                let stream = incoming.next().await.expect("client should connect");
                debug!("server {} got connection. waiting", i);
                let mut tcp_stream = stream?;

                debug!(
                    "server {}, send back file using zero copy: {:#?}",
                    i,
                    f_slice.len()
                );
                tcp_stream
                    .zero_copy_write(&f_slice)
                    .await
                    .expect("file slice");
            }

            Ok(()) as Result<(), SendFileError>
        };

        let client = async {
            sleep(time::Duration::from_millis(100)).await;
            let addr = format!("127.0.0.1:{}", ZERO_COPY_PORT)
                .parse::<SocketAddr>()
                .expect("parse");
            debug!("client: file loaded");

            for i in 0..TEST_ITERATION {
                debug!("client: Test loop: {}", i);
                let mut stream = TcpStream::connect(&addr).await.expect("unable to connect");
                debug!("client: {} connected trying to read", i);
                let mut buffer = Vec::with_capacity(MAX_BYTES);
                stream
                    .read_exact(&mut buffer)
                    .await
                    .expect("no more buffer");
                debug!("client: {} test success", i);

                // sleep 10 milliseconds between request to keep tcp connection otherwise it may lead to EPIPE error
                //
                sleep(time::Duration::from_millis(10)).await;
            }

            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream
        let _ = zip(client, server).await;
        Ok(())
    }
}
