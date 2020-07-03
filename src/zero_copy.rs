use std::io::Error as IoError;
use std::fmt;


use std::os::unix::io::AsRawFd;

use nix::sys::sendfile::sendfile;
#[allow(unused)]
use nix::libc::off_t;
use nix::Error as NixError;
use async_trait::async_trait;

use crate::task::spawn_blocking;


use crate::fs::AsyncFileSlice;

#[derive(Debug)]
pub enum SendFileError {
    IoError(IoError),
    NixError(NixError),
}

impl fmt::Display for SendFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::IoError(err) => write!(f, "{}", err),
            Self::NixError(err) => write!(f, "{}", err),
        }
    }
}

impl From<IoError> for SendFileError {
    fn from(error: IoError) -> Self {
        SendFileError::IoError(error)
    }
}

impl From<NixError> for SendFileError {
    fn from(error: NixError) -> Self {
        SendFileError::NixError(error)
    }
}

/// zero copy write
#[async_trait]
pub trait ZeroCopyWrite  {
    async fn zero_copy_write(&mut self, source: &AsyncFileSlice) -> Result<usize, SendFileError>;
}

#[async_trait]
impl <T> ZeroCopyWrite for T where T: AsRawFd + Send {

    async fn zero_copy_write(&mut self, source: &AsyncFileSlice) -> Result<usize, SendFileError> {
        let size = source.len();
        let target_fd = self.as_raw_fd();
        let source_fd = source.fd();

        #[cfg(target_os = "linux")]
        let ft = {
            let offset = source.position() as off_t;

            spawn_blocking(move || {

                let mut total_transferred: usize = 0;           // total bytes transferred so far
                let mut current_offset = offset;
            
                loop {

                    let to_be_transfer = size as usize - total_transferred;

                    log::trace!(
                        "trying: zero copy source fd: {} offset: {} len: {}, target: fd{}",
                        source_fd,
                        current_offset,
                        to_be_transfer,
                        target_fd
                    );
    
                    match sendfile(target_fd, source_fd, Some(&mut current_offset), to_be_transfer) {
                        Ok(len) => {
    
                            total_transferred += len as usize;
                            current_offset += len as off_t;
                            log::trace!("actual: zero copy bytes transferred: {} out of {}", len, size);
                            
                            if total_transferred < size as usize {
                                log::debug!("current transferred: {} less than total: {}, continuing",total_transferred,size);
                            } else {
                                return Ok(len as usize)
                            }
                        },
                        Err(err) => {
                            log::error!("error sendfile: {}", err);
                            return Err(err.into())
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

                loop  {

                    let to_be_transfer = (size - total_transferred) as i64;


                    log::trace!(
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

                    log::trace!("mac zero copy bytes transferred: {}", len);
                    total_transferred += len as u64;
                    current_offset += len as u64;
                    match res {
                        Ok(_) => {
                            if total_transferred < size  {
                                log::debug!("current transferred: {} less than total: {}, continuing",total_transferred,size);
                            } else {
                                return Ok(len as usize)
                            }
                          
                        }
                        Err(err) => {
                            match err {
                                NixError::Sys(err_no) => {
                                    if err_no == Errno::EAGAIN {
                                        log::debug!("EAGAIN, try again");
                                        continue;
                                    }
                                }
                                _ => {}
                            }
                            
                            log::error!("error sendfile: {}", err);
                            return  Err(err.into());
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

    use log::debug;
    use futures::stream::StreamExt;    
    use futures::future::join;

    use crate::net::TcpStream;
    use crate::net::TcpListener;
    use crate::io::AsyncReadExt;
    use crate::fs::util as file_util;
    use crate::zero_copy::ZeroCopyWrite;
    use crate::fs::AsyncFile;
    use crate::timer::sleep;
    use crate::test_async;

    use super::SendFileError;

    #[test_async]
    async fn test_zero_copy_from_fs_to_socket() -> Result<(), SendFileError> {
        // spawn tcp client and check contents
        let server = async {

            #[allow(unused_mut)]
            let mut listener = TcpListener::bind("127.0.0.1:9999").await?;
            
            debug!("server: listening");
            let mut incoming = listener.incoming();
            if let Some(stream) = incoming.next().await {
                debug!("server: got connection. waiting");
                let mut tcp_stream = stream?;
                let mut buf = [0; 30];
                let len = tcp_stream.read(&mut buf).await?;
                assert_eq!(len, 30);
            } else {
                assert!(false, "client should connect");
            }
            Ok(()) as Result<(), SendFileError>
        };

        let client = async {
            let file = file_util::open("test-data/apirequest.bin").await?;
            sleep(time::Duration::from_millis(100)).await;
            let addr = "127.0.0.1:9999".parse::<SocketAddr>().expect("parse");
            debug!("client: file loaded");
            let mut stream = TcpStream::connect(&addr).await?;
            debug!("client: connected to server");
            let f_slice = file.as_slice(0, None).await?;
            debug!("client: send back file using zero copy");
            stream.zero_copy_write(&f_slice).await?;
            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream

        let _rt = join(client, server).await;
        Ok(())
    }


    #[test_async]
    async fn test_zero_copy_large_size() -> Result<(), SendFileError> {

        const MAX_BYTES: usize = 300000;
        const PORT: u16 = 8888;

        use std::env::temp_dir;
        use crate::io::AsyncWriteExt;

        const TEST_ITERATION: u16 = 20;

        async fn init_file() {
            let temp_file = temp_dir().join("async_large");
            debug!("temp file: {:#?}",temp_file);
            let mut file = file_util::create(temp_file.clone()).await.expect("file creation");
            
            let bytes: Vec<u8> = vec![0;1000];
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
            assert_eq!(f_slice.len(),MAX_BYTES as u64);

            let listener = TcpListener::bind(format!("127.0.0.1:{}",PORT)).await.expect("failed bind");
            
            debug!("server: listening");
            let mut incoming = listener.incoming();

            for i in 0..TEST_ITERATION {

                if let Some(stream) = incoming.next().await {
                    debug!("server {} got connection. waiting",i);
                    let mut tcp_stream = stream?;
                               
                    debug!("server {}, send back file using zero copy: {:#?}",i,f_slice.len());
                    tcp_stream.zero_copy_write(&f_slice).await.expect("file slice");

                } else {
                    assert!(false, "client should connect");
                }

            }
            
            Ok(()) as Result<(), SendFileError>
        };

        let client = async {
            
            sleep(time::Duration::from_millis(100)).await;
            let addr = format!("127.0.0.1:{}",PORT).parse::<SocketAddr>().expect("parse");
            debug!("client: file loaded");
            

            for i in 0..TEST_ITERATION {
                debug!("client: Test loop: {}",i);
                let mut stream = TcpStream::connect(&addr).await?;
                debug!("client: {} connected trying to read",i);
                let mut buffer = Vec::with_capacity(MAX_BYTES);
                stream.read_exact(&mut buffer).await.expect("no more buffer");
                debug!("client: {} test success",i);

                // sleep 10 milliseconds between request to keep tcp connection otherwise it may lead to EPIPE error
                //  
                sleep(time::Duration::from_millis(10)).await;
            }

            Ok(()) as Result<(), SendFileError>
        };

        // read file and zero copy to tcp stream

        let _rt = join(client, server).await;
        Ok(())
    }
}
