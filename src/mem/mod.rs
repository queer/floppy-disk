use std::io::Result;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::SystemTime;

use derivative::Derivative;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};

use crate::FloppyDisk;
use crate::{FloppyFile, FloppyFileType, FloppyMetadata, FloppyPermissions};

use self::inode::{Inode, InodeType};

mod inode;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFloppyDisk {}

impl MemFloppyDisk {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl FloppyDisk for MemFloppyDisk {
    type Metadata = MemMetadata;

    type ReadDir = crate::tokio_fs::TokioReadDir;

    type Permissions = MemPermissions;

    async fn canonicalize<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        todo!()
    }

    async fn copy<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<u64> {
        todo!()
    }

    async fn create_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        todo!()
    }

    async fn create_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        todo!()
    }

    async fn hard_link<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()> {
        todo!()
    }

    async fn metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        todo!()
    }

    async fn read<P: AsRef<Path> + Send>(&self, path: P) -> Result<Vec<u8>> {
        todo!()
    }

    async fn read_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::ReadDir> {
        todo!()
    }

    async fn read_link<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        todo!()
    }

    async fn read_to_string<P: AsRef<Path> + Send>(&self, path: P) -> Result<String> {
        todo!()
    }

    async fn remove_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        todo!()
    }

    async fn remove_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        todo!()
    }

    async fn remove_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        todo!()
    }

    async fn rename<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<()> {
        todo!()
    }

    async fn set_permissions<P: AsRef<Path> + Send>(
        &self,
        path: P,
        perm: Self::Permissions,
    ) -> Result<()> {
        todo!()
    }

    async fn symlink<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()> {
        todo!()
    }

    async fn symlink_metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        todo!()
    }

    async fn try_exists<P: AsRef<Path> + Send>(&self, path: P) -> Result<bool> {
        todo!()
    }

    async fn write<P: AsRef<Path> + Send>(
        &self,
        path: P,
        contents: impl AsRef<[u8]> + Send,
    ) -> Result<()> {
        todo!()
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFile {
    inode: Arc<RwLock<Inode>>,
    position: u64,
}

#[async_trait::async_trait]
impl FloppyFile for MemFile {
    type Metadata = MemMetadata;
    type Permissions = MemPermissions;

    async fn sync_all(&mut self) -> Result<()> {
        Ok(())
    }

    async fn sync_data(&mut self) -> Result<()> {
        Ok(())
    }

    async fn set_len(&mut self, size: u64) -> Result<()> {
        let mut inode = self.inode.write().unwrap();
        inode.set_len(size);
        Ok(())
    }

    async fn metadata(&self) -> Result<Self::Metadata> {
        Ok(Self::Metadata {
            inode: self.inode.clone(),
        })
    }

    async fn try_clone(&self) -> Result<Box<Self>> {
        Ok(Box::new(Self {
            inode: self.inode.clone(),
            position: self.position,
        }))
    }

    async fn set_permissions(&self, perm: Self::Permissions) -> Result<()> {
        let mut inode = self.inode.write().unwrap();
        inode.set_permissions(perm);
        Ok(())
    }

    async fn permissions(&self) -> Result<Self::Permissions> {
        let inode = self.inode.read().unwrap();
        Ok(inode.permissions())
    }
}

impl AsyncSeek for MemFile {
    fn start_seek(self: Pin<&mut Self>, position: std::io::SeekFrom) -> Result<()> {
        let mut this = self.get_mut();

        match position {
            std::io::SeekFrom::Start(pos) => this.position = pos,
            std::io::SeekFrom::End(pos) => {
                let inode = this.inode.read().unwrap();
                this.position = (inode.len() as i64 + pos) as u64;

                if this.position > inode.len() {
                    this.position = inode.len();
                }
            }
            std::io::SeekFrom::Current(pos) => this.position = (this.position as i64 + pos) as u64,
        }

        Ok(())
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<u64>> {
        let this = self.get_mut();
        std::task::Poll::Ready(Ok(this.position))
    }
}

impl AsyncRead for MemFile {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<Result<()>> {
        let mut this = self.get_mut();
        let inode = this.inode.read().unwrap();
        let buffer = inode.buffer();

        let len = buf.remaining();
        let position = this.position as usize;

        if position >= buffer.len() {
            return std::task::Poll::Ready(Ok(()));
        }

        let end = std::cmp::min(position + len, buffer.len());
        let slice = &buffer[position..end];
        buf.put_slice(slice);
        this.position += slice.len() as u64;

        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for MemFile {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize>> {
        let this = self.get_mut();
        let mut inode = this.inode.write().unwrap();

        let position = this.position;
        let len = buf.len();

        let buffer = inode.buffer_mut();
        buffer.resize(position as usize + len, 0);
        buffer[position as usize..position as usize + len].copy_from_slice(buf);

        this.position += len as u64;

        std::task::Poll::Ready(Ok(len))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemPermissions {
    mode: u32,
}

impl FloppyPermissions for MemPermissions {
    fn readonly(&self) -> bool {
        self.mode & 0o222 == 0
    }

    fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.mode &= !0o222;
        } else {
            self.mode |= 0o222;
        }
    }
}

#[derive(Debug)]
pub struct MemMetadata {
    inode: Arc<RwLock<Inode>>,
}

#[async_trait::async_trait]
impl FloppyMetadata for MemMetadata {
    type FileType = MemFileType;

    type Permissions = MemPermissions;

    async fn file_type(&self) -> Self::FileType {
        let inode = self.inode.read().unwrap();
        match inode.kind() {
            InodeType::File => MemFileType(InodeType::File),
            InodeType::Dir => MemFileType(InodeType::Dir),
            InodeType::Symlink => MemFileType(InodeType::Symlink),
        }
    }

    async fn is_dir(&self) -> bool {
        let inode = self.inode.read().unwrap();
        *inode.kind() == InodeType::Dir
    }

    async fn is_file(&self) -> bool {
        let inode = self.inode.read().unwrap();
        *inode.kind() == InodeType::File
    }

    async fn is_symlink(&self) -> bool {
        let inode = self.inode.read().unwrap();
        *inode.kind() == InodeType::Symlink
    }

    async fn len(&self) -> u64 {
        let inode = self.inode.read().unwrap();
        inode.len()
    }

    async fn permissions(&self) -> Self::Permissions {
        let inode = self.inode.read().unwrap();
        inode.mode().clone()
    }

    async fn modified(&self) -> Result<SystemTime> {
        let inode = self.inode.read().unwrap();
        Ok(*inode.mtime())
    }

    async fn accessed(&self) -> Result<SystemTime> {
        let inode = self.inode.read().unwrap();
        Ok(*inode.atime())
    }

    async fn created(&self) -> Result<SystemTime> {
        let inode = self.inode.read().unwrap();
        Ok(*inode.ctime())
    }
}

#[derive(Debug)]
pub struct MemFileType(#[doc(hidden)] InodeType);

impl FloppyFileType for MemFileType {
    fn is_dir(&self) -> bool {
        self.0 == InodeType::Dir
    }

    fn is_file(&self) -> bool {
        self.0 == InodeType::File
    }

    fn is_symlink(&self) -> bool {
        self.0 == InodeType::Symlink
    }
}
