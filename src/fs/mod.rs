mod bounded;
mod extension;

pub use extension::*;

pub use self::bounded::BoundedFileOption;
// pub use self::bounded::BoundedFileSink;
pub use self::bounded::BoundedFileSinkError;

#[cfg(feature = "mmap")]
pub mod mmap;

// pub use async_fs::*;
pub use tokio::fs::*;

pub mod util {

    use std::io::Error as IoError;
    use std::path::Path;

    use super::File;
    use super::OpenOptions;

    /// open for write only
    pub async fn create<P>(path: P) -> Result<File, IoError>
    where
        P: AsRef<Path>,
    {
        File::create(path.as_ref()).await
    }

    /// open for only read
    pub async fn open<P>(path: P) -> Result<File, IoError>
    where
        P: AsRef<Path>,
    {
        let file_path = path.as_ref();
        File::open(file_path).await
    }

    /// open for read and write
    pub async fn open_read_write<P>(path: P) -> Result<File, IoError>
    where
        P: AsRef<Path>,
    {
        let file_path = path.as_ref();
        let mut option = OpenOptions::new();
        option.read(true).write(true).create(true).append(false);

        option.open(file_path).await
    }

    pub async fn open_read_append<P>(path: P) -> Result<File, IoError>
    where
        P: AsRef<Path>,
    {
        let file_path = path.as_ref();
        let mut option = OpenOptions::new();
        option.read(true).create(true).append(true);

        option.open(file_path).await
    }
}
