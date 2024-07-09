use std::io::Error as IoError;
use std::os::fd::BorrowedFd;
use std::os::unix::io::{AsRawFd, RawFd};
use std::thread::sleep;
use thiserror::Error;

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
pub struct ZeroCopy(RawFd);

impl ZeroCopy {
    pub fn from<S>(fd: &mut S) -> Self
    where
        S: AsRawFd,
    {
        Self(fd.as_raw_fd())
    }

    pub fn raw(fd: RawFd) -> Self {
        Self(fd)
    }
}

impl ZeroCopy {
    pub async fn copy_slice(&self, source: &AsyncFileSlice) -> Result<usize, SendFileError> {
        let size = source.len();
        let target_raw_fd = self.0;
        let source_raw_fd = source.fd();

        #[cfg(target_os = "linux")]
        let ft = {
            let offset = source.position() as off_t;

            spawn_blocking(move || {
                let mut total_transferred: usize = 0; // total bytes transferred so far
                let mut current_offset = offset;

                let target_fd = unsafe { BorrowedFd::borrow_raw(target_raw_fd) };
                let source_fd = unsafe { BorrowedFd::borrow_raw(source_raw_fd) };

                loop {
                    let to_be_transfer = size as usize - total_transferred;

                    trace!(
                        "trying: zero copy source fd: {} offset: {} len: {}, target fd: {}",
                        source_raw_fd,
                        current_offset,
                        to_be_transfer,
                        target_raw_fd
                    );

                    match sendfile(
                        target_fd,
                        source_fd,
                        Some(&mut current_offset),
                        to_be_transfer,
                    ) {
                        Ok(bytes_transferred) => {
                            trace!("bytes transferred: {}", bytes_transferred);
                            total_transferred += bytes_transferred;

                            // zero bytes transferred means it's EOF
                            if bytes_transferred == 0 {
                                return Ok(total_transferred);
                            }

                            if total_transferred < size as usize {
                                debug!(
                                    "current transferred: {} less than total: {}, continuing",
                                    total_transferred, size
                                );
                            } else {
                                trace!(
                                    "actual: zero copy bytes transferred: {} out of {}, ",
                                    bytes_transferred,
                                    size
                                );

                                return Ok(total_transferred);
                            }
                        }
                        Err(err) => match err {
                            nix::errno::Errno::EAGAIN => {
                                debug!(
                                    "EAGAIN, continuing source: {},target: {}",
                                    source_raw_fd, target_raw_fd
                                );
                                sleep(std::time::Duration::from_millis(10));
                            }
                            _ => {
                                log::error!("error sendfile: {}", err);
                                return Err(err.into());
                            }
                        },
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
                let mut current_offset = offset;

                let target_fd = unsafe { BorrowedFd::borrow_raw(target_raw_fd) };
                let source_fd = unsafe { BorrowedFd::borrow_raw(source_raw_fd) };

                loop {
                    let to_be_transfer = (size - total_transferred) as i64;

                    trace!(
                        "mac zero copy source fd: {} offset: {} len: {}, target: fd{}",
                        source_raw_fd,
                        current_offset,
                        to_be_transfer,
                        target_raw_fd
                    );

                    let (res, bytes_transferred) = sendfile(
                        source_fd,
                        target_fd,
                        current_offset as i64,
                        Some(to_be_transfer),
                        None,
                        None,
                    );

                    trace!("mac zero copy bytes transferred: {}", bytes_transferred);

                    total_transferred += bytes_transferred as u64;
                    current_offset += bytes_transferred as u64;
                    match res {
                        Ok(_) => {
                            // zero bytes transferred means it's EOF
                            if bytes_transferred == 0 {
                                trace!("no more bytes transferred");
                                return Ok(total_transferred as usize);
                            }
                            if total_transferred < size {
                                debug!(
                                    "current transferred: {} less than total: {}, continuing",
                                    total_transferred, size
                                );
                            } else {
                                return Ok(total_transferred as usize);
                            }
                        }
                        Err(err) => {
                            if err == Errno::EAGAIN {
                                debug!("EAGAIN, try again");
                                sleep(std::time::Duration::from_millis(10));
                            } else {
                                log::error!("error sendfile: {}", err);
                                return Err(err.into());
                            }
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

    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time;

    use futures_lite::future::zip;
    use futures_util::stream::StreamExt;
    use log::debug;

    use crate::file_slice::AsyncFileSlice;
    use crate::fs::AsyncFileExtension;
    use crate::net::tcp_stream::stream;
    use crate::net::TcpListener;
    use crate::test_async;
    use crate::timer::sleep;
    use crate::{fs::util as file_util, zero_copy::ZeroCopy};
    use futures_lite::AsyncReadExt;

    use super::SendFileError;

    #[test_async]
    async fn test_zero_copy_simple() {
        let port = portpicker::pick_unused_port().expect("No free ports left");
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

        // spawn tcp client and check contents
        let server = async {
            #[allow(unused_mut)]
            let mut listener = TcpListener::bind(addr).await?;

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

            debug!("client: file loaded");
            let mut stream = stream(&addr).await?;
            debug!("client: connected to server");
            let f_slice = file.as_slice(0, None).await?;
            debug!("client: send back file using zero copy");
            let writer = ZeroCopy::from(&mut stream);
            let transfered = writer.copy_slice(&f_slice).await?;
            assert_eq!(transfered, 30);
            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream

        let _ = zip(client, server).await;
    }

    #[test_async]
    async fn test_zero_copy_large_size() {
        const MAX_BYTES: usize = 3000000;

        use futures_lite::AsyncWriteExt;
        use std::env::temp_dir;

        const TEST_ITERATION: u16 = 2;

        let port = portpicker::pick_unused_port().expect("No free ports left");
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

        async fn init_file() {
            let temp_file = temp_dir().join("async_large");
            debug!("temp file: {:#?}", temp_file);
            let mut file = file_util::create(temp_file.clone())
                .await
                .expect("file creation");

            let bytes: Vec<u8> = vec![0; 1000];
            for _ in 0..3000 {
                file.write_all(&bytes).await.expect("writing");
            }

            file.sync_all().await.expect("flushing");
            drop(file);
            debug!("finish creating large test file");
        }

        // spawn tcp client and check contents
        let server = async {
            let temp_file = temp_dir().join("async_large");
            // do zero copy
            let file = file_util::open(&temp_file).await.expect("re opening");
            let f_slice = file.as_slice(0, None).await.expect("filed opening");
            assert_eq!(f_slice.len(), MAX_BYTES as u64);

            let listener = TcpListener::bind(addr).await.expect("failed bind");

            debug!("server: listening");
            let mut incoming = listener.incoming();

            for i in 0..TEST_ITERATION {
                let stream = incoming.next().await.expect("client should connect");
                debug!("server {} got connection. waiting", i);

                let mut tcp_stream = stream.expect("stream");

                debug!(
                    "server {}, send back file using zero copy: {:#?}",
                    i,
                    f_slice.len()
                );

                let writer = ZeroCopy::from(&mut tcp_stream);

                if i == 0 {
                    let transferred = writer.copy_slice(&f_slice).await.expect("file slice");
                    assert_eq!(transferred, MAX_BYTES);
                } else {
                    let slice2 = AsyncFileSlice::new(f_slice.fd(), 0, (MAX_BYTES * 2) as u64);
                    let transferred = writer.copy_slice(&slice2).await.expect("file slice");
                    assert_eq!(transferred, MAX_BYTES);
                }
            }
        };

        let client = async {
            // wait 1 seconds to give server to be ready
            sleep(time::Duration::from_millis(100)).await;
            debug!("client loop starting");

            for i in 0..TEST_ITERATION {
                debug!("client: Test loop: {}", i);
                let mut stream = stream(&addr).await?;
                debug!("client: {} connected trying to read", i);
                // let server send response

                let mut buffer = Vec::with_capacity(MAX_BYTES);
                stream
                    .read_to_end(&mut buffer)
                    .await
                    .expect("no more buffer");
                debug!("client: {} test success", i);

                // sleep 10 milliseconds between request to keep tcp connection otherwise it may lead to EPIPE error
                //
                sleep(time::Duration::from_millis(10)).await;
            }

            Ok(()) as Result<(), SendFileError>
        };

        init_file().await;

        // read file and zero copy to tcp stream
        let _ = zip(client, server).await;
    }

    /// test zero copy when len is too large
    #[test_async]
    async fn test_zero_copy_large_slace() {
        let port = portpicker::pick_unused_port().expect("No free ports left");
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

        // spawn tcp client and check contents
        let server = async {
            #[allow(unused_mut)]
            let mut listener = TcpListener::bind(addr).await?;

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

            debug!("client: file loaded");
            let mut stream = stream(&addr).await?;
            debug!("client: connected to server");
            let f_slice = file.as_slice(0, None).await?;
            let max_slice = AsyncFileSlice::new(f_slice.fd(), 0, 1000);
            debug!("slice: {:#?}", max_slice);
            debug!("client: send back file using zero copy");
            let writer = ZeroCopy::from(&mut stream);
            let transfer = writer.copy_slice(&max_slice).await?;
            assert_eq!(transfer, 30);
            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream

        let _ = zip(client, server).await;
    }
}
