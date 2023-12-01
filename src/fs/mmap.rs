// memory mapped file

use std::fs::OpenOptions;
use std::io::Error as IoError;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

use async_fs::File;
use memmap2::Mmap;
use memmap2::MmapMut;

use crate::task::spawn_blocking;

/// Mutable async wrapper for MmapMut
pub struct MemoryMappedMutFile(Arc<RwLock<MmapMut>>);

impl MemoryMappedMutFile {
    pub async fn create(m_path: &Path, len: u64) -> Result<(Self, File), IoError> {
        let owned_path = m_path.to_owned();
        let (m_map, mfile, _) = spawn_blocking(move || {
            let inner_path = owned_path.clone();
            let mfile = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(inner_path)
                .unwrap();

            mfile.set_len(len)?;

            unsafe { MmapMut::map_mut(&mfile) }.map(|mm_file| (mm_file, mfile, owned_path))
        })
        .await?;

        Ok((MemoryMappedMutFile::from_mmap(m_map), mfile.into()))
    }

    fn from_mmap(mmap: MmapMut) -> MemoryMappedMutFile {
        MemoryMappedMutFile(Arc::new(RwLock::new(mmap)))
    }

    pub fn inner(&self) -> RwLockReadGuard<MmapMut> {
        self.0.read().unwrap()
    }

    pub fn inner_map(&self) -> Arc<RwLock<MmapMut>> {
        self.0.clone()
    }

    pub fn mut_inner(&self) -> RwLockWriteGuard<MmapMut> {
        self.0.write().unwrap()
    }

    /// write bytes at location,
    /// return number bytes written
    pub fn write_bytes(&mut self, pos: usize, bytes: &[u8]) {
        let mut m_file = self.mut_inner();
        let m_array = &mut m_file[..];
        m_array[pos..(pos + bytes.len())].clone_from_slice(bytes);
    }

    pub async fn flush_ft(&self) -> Result<(), IoError> {
        let inner = self.0.clone();
        spawn_blocking(move || {
            let inner_map = inner.write().unwrap();
            let res = inner_map.flush();
            drop(inner_map);
            res
        })
        .await
    }

    pub async fn flush_async_ft(&self) -> Result<(), IoError> {
        let inner = self.0.clone();
        spawn_blocking(move || {
            let inner_map = inner.write().unwrap();
            inner_map.flush_async()
        })
        .await
    }

    pub async fn flush_range_ft(&self, offset: usize, len: usize) -> Result<(), IoError> {
        let inner = self.0.clone();
        spawn_blocking(move || {
            let inner_map = inner.write().unwrap();
            inner_map.flush_range(offset, len)
        })
        .await
    }
}

/// Async wrapper for read only mmap
pub struct MemoryMappedFile(Arc<RwLock<Mmap>>);

impl MemoryMappedFile {
    /// open memory file, specify minimum size
    pub async fn open<P>(path: P, min_len: u64) -> Result<(Self, File), IoError>
    where
        P: AsRef<Path>,
    {
        let m_path = path.as_ref().to_owned();
        let (m_map, mfile, _) = spawn_blocking(move || {
            let mfile = OpenOptions::new().read(true).open(&m_path).unwrap();
            let meta = mfile.metadata().unwrap();
            if meta.len() == 0 {
                let fd = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&m_path)
                    .unwrap();

                fd.set_len(min_len)?;
            }

            unsafe { Mmap::map(&mfile) }.map(|mm_file| (mm_file, mfile, m_path))
        })
        .await?;

        Ok((MemoryMappedFile::from_mmap(m_map), mfile.into()))
    }

    fn from_mmap(mmap: Mmap) -> MemoryMappedFile {
        MemoryMappedFile(Arc::new(RwLock::new(mmap)))
    }

    pub fn inner(&self) -> RwLockReadGuard<Mmap> {
        self.0.read().unwrap()
    }
}

#[cfg(test)]
mod tests {

    use std::env::temp_dir;
    use std::fs::File;
    use std::io::Error as IoError;
    use std::io::Read;

    use async_fs::OpenOptions;
    use flv_util::fixture::ensure_clean_file;

    use crate::test_async;

    use super::{MemoryMappedFile, MemoryMappedMutFile};

    #[test_async]
    async fn test_mmap_write_slice() -> Result<(), IoError> {
        let index_path = temp_dir().join("test.index");
        ensure_clean_file(index_path.clone());

        let result = MemoryMappedMutFile::create(&index_path, 3).await;
        assert!(result.is_ok());

        let (mm_file, _) = result.unwrap();
        {
            let mut mm = mm_file.mut_inner();
            let src = [0x01, 0x02, 0x03];
            mm.copy_from_slice(&src);
        }

        mm_file.flush_ft().await?;

        let mut f = File::open(&index_path)?;
        let mut buffer = vec![0; 3];
        f.read_exact(&mut buffer)?;
        assert_eq!(buffer[0], 0x01);
        assert_eq!(buffer[1], 0x02);
        assert_eq!(buffer[2], 0x03);

        Ok(())
    }

    #[test_async]
    async fn test_mmap_write_pair_slice() -> Result<(), IoError> {
        let index_path = temp_dir().join("pairslice.index");
        ensure_clean_file(index_path.clone());

        let result = MemoryMappedMutFile::create(&index_path, 24).await;
        assert!(result.is_ok());

        let (mm_file, _) = result.unwrap();
        {
            let mut mm = mm_file.mut_inner();
            let src: [(u32, u32); 3] = [(5, 10), (11, 22), (50, 100)];
            let (_, bytes, _) = unsafe { src.align_to::<u8>() };
            assert_eq!(bytes.len(), 24);
            mm.copy_from_slice(bytes);
        }

        mm_file.flush_ft().await?;

        let (mm_file2, _) = MemoryMappedMutFile::create(&index_path, 24).await?;
        let mm2 = mm_file2.mut_inner();
        let (_, pairs, _) = unsafe { mm2.align_to::<(u32, u32)>() };
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].0, 5);
        assert_eq!(pairs[2].1, 100);

        Ok(())
    }

    #[test_async]
    async fn test_mmap_write_with_pos() -> Result<(), IoError> {
        let index_path = temp_dir().join("testpos.index");
        ensure_clean_file(index_path.clone());

        let (mut mm_file, _) = MemoryMappedMutFile::create(&index_path, 10).await?;

        let src = vec![0x05, 0x10, 0x44];
        mm_file.write_bytes(5, &src);

        mm_file.flush_ft().await?;

        let mut f = File::open(&index_path)?;
        let mut buffer = vec![0; 10];
        f.read_exact(&mut buffer)?;
        assert_eq!(buffer[5], 0x05);
        assert_eq!(buffer[6], 0x10);
        assert_eq!(buffer[7], 0x44);

        Ok(())
    }

    #[test_async]
    async fn test_empty_index_read_only() -> Result<(), IoError> {
        let index_path = temp_dir().join("zerosized.index");
        ensure_clean_file(index_path.clone());
        {
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .read(true)
                .open(&index_path)
                .await?;
            let meta = file.metadata().await?;
            assert_eq!(meta.len(), 0);
        }

        let min_size = 10;

        let (mm_file, _) = MemoryMappedFile::open(&index_path, min_size).await?;
        let memory_size = mm_file.inner().len();
        assert_eq!(memory_size, min_size as usize);

        Ok(())
    }

    /*
    use std::fs::OpenOptions;
    use std::path::PathBuf;
    use memmap::MmapMut;


    #[test]
    fn debug_kafka_inspect() -> io::Result<()>  {

        let path =  "/tmp/kafka-logs/test-0/00000000000000000000.index";
        let file = OpenOptions::new()
                       .read(true)
                       .write(true)
                       .open(path)?;

        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        println!("file size: {}",mmap.len());
        Ok(())
    }

    #[test]
    fn debug_file_inspect() -> io::Result<()>  {

        let path =  "/tmp/kafka-logs/test-0/00000000000000000000.index";
        let file = File::open(path)?;
        let metadata = file.metadata()?;

        println!("file len: {:#?}",metadata.len());
        Ok(())
    }
    */
}
