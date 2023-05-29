//! DIY: `#[derive(Clone, Debug)]`

use std::ffi::OsString;
use std::fmt::Debug;
use std::io::Result;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};

pub mod mem;
pub mod tokio_fs;

pub mod prelude {
    pub use crate::{
        FloppyDirBuilder, FloppyDirEntry, FloppyDisk, FloppyDiskUnixExt, FloppyFile,
        FloppyFileType, FloppyMetadata, FloppyOpenOptions, FloppyPermissions, FloppyReadDir,
        FloppyUnixMetadata, FloppyUnixPermissions,
    };

    pub use crate::mem::MemFloppyDisk;
    pub use crate::tokio_fs::TokioFloppyDisk;
}

#[async_trait::async_trait]
pub trait FloppyDisk<'a>: Debug + std::marker::Unpin + std::marker::Sized + Send {
    type DirBuilder: FloppyDirBuilder + Send + 'a;
    type DirEntry: FloppyDirEntry<'a, Self> + Send + 'a;
    type File: FloppyFile<'a, Self> + Send + 'a;
    type FileType: FloppyFileType + Send + 'a;
    type Metadata: FloppyMetadata<'a, Self> + Send + 'a;
    type OpenOptions: FloppyOpenOptions<'a, Self> + Send + 'a;
    type Permissions: FloppyPermissions + Send + 'a;
    type ReadDir: FloppyReadDir<'a, Self> + Send + 'a;
    // type TempDir: FloppyTempDir;

    async fn canonicalize<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf>;

    async fn copy<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<u64>;

    async fn create_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;

    async fn create_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;

    async fn hard_link<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()>;

    async fn metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata>;

    async fn read<P: AsRef<Path> + Send>(&self, path: P) -> Result<Vec<u8>>;

    async fn read_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::ReadDir>;

    async fn read_link<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf>;

    async fn read_to_string<P: AsRef<Path> + Send>(&self, path: P) -> Result<String>;

    async fn remove_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;

    async fn remove_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;

    async fn remove_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;

    async fn rename<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<()>;

    async fn set_permissions<P: AsRef<Path> + Send>(
        &mut self,
        path: P,
        perm: Self::Permissions,
    ) -> Result<()>;

    async fn symlink<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()>;

    async fn symlink_metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata>;

    async fn try_exists<P: AsRef<Path> + Send>(&self, path: P) -> Result<bool>;

    async fn write<P: AsRef<Path> + Send>(
        &self,
        path: P,
        contents: impl AsRef<[u8]> + Send,
    ) -> Result<()>;

    fn new_dir_builder(&'a self) -> Self::DirBuilder;
}

#[async_trait::async_trait]
pub trait FloppyDiskUnixExt {
    async fn chown<P: Into<PathBuf> + Send>(&self, path: P, uid: u32, gid: u32) -> Result<()>;
}

#[allow(clippy::len_without_is_empty)]
#[async_trait::async_trait]
pub trait FloppyMetadata<'a, Disk: FloppyDisk<'a>>: Debug + std::marker::Unpin + Send {
    fn file_type(&self) -> Disk::FileType;
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_symlink(&self) -> bool;
    fn len(&self) -> u64;
    fn permissions(&self) -> Disk::Permissions;
    fn modified(&self) -> Result<SystemTime>;
    fn accessed(&self) -> Result<SystemTime>;
    fn created(&self) -> Result<SystemTime>;
}

#[async_trait::async_trait]
pub trait FloppyUnixMetadata {
    fn uid(&self) -> Result<u32>;
    fn gid(&self) -> Result<u32>;
}

#[async_trait::async_trait]
pub trait FloppyReadDir<'a, Disk: FloppyDisk<'a>>: Debug + std::marker::Unpin + Send {
    async fn next_entry(&mut self) -> Result<Option<Disk::DirEntry>>;
}

pub trait FloppyPermissions: Debug + std::marker::Unpin + Send {
    fn readonly(&self) -> bool;
    fn set_readonly(&mut self, readonly: bool);
}

pub trait FloppyUnixPermissions: Debug + std::marker::Unpin + Send {
    fn mode(&self) -> u32;
    fn set_mode(&mut self, mode: u32);
    fn from_mode(mode: u32) -> Self;
}

#[async_trait::async_trait]
pub trait FloppyDirBuilder: Debug + std::marker::Unpin + Send {
    fn recursive(&mut self, recursive: bool) -> &mut Self;
    async fn create<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;
    #[cfg(unix)]
    fn mode(&mut self, mode: u32) -> &mut Self;
}

#[async_trait::async_trait]
pub trait FloppyDirEntry<'a, Disk: FloppyDisk<'a>>: Debug + std::marker::Unpin + Send {
    fn path(&self) -> PathBuf;
    fn file_name(&self) -> OsString;
    async fn metadata(&self) -> Result<Disk::Metadata>;
    async fn file_type(&self) -> Result<Disk::FileType>;

    #[cfg(unix)]
    fn ino(&self) -> u64;
}

#[async_trait::async_trait]
pub trait FloppyFile<'a, Disk: FloppyDisk<'a>>:
    AsyncRead + AsyncWrite + AsyncSeek + Debug + std::marker::Unpin + Send
{
    async fn sync_all(&mut self) -> Result<()>;
    async fn sync_data(&mut self) -> Result<()>;
    async fn set_len(&mut self, size: u64) -> Result<()>;
    async fn metadata(&self) -> Result<Disk::Metadata>;
    async fn try_clone(&'a self) -> Result<Box<Disk::File>>;
    async fn set_permissions(&self, perm: Disk::Permissions) -> Result<()>;
    async fn permissions(&self) -> Result<Disk::Permissions>;
}

#[async_trait::async_trait]
pub trait FloppyOpenOptions<'a, Disk: FloppyDisk<'a>>: Debug + std::marker::Unpin + Send {
    fn new() -> Self;
    fn read(self, read: bool) -> Self;
    fn write(self, write: bool) -> Self;
    fn append(self, append: bool) -> Self;
    fn truncate(self, truncate: bool) -> Self;
    fn create(self, create: bool) -> Self;
    fn create_new(self, create_new: bool) -> Self;
    async fn open<P: AsRef<Path> + Send>(&self, disk: &'a Disk, path: P) -> Result<Disk::File>;
}

pub trait FloppyFileType: Debug + std::marker::Unpin + Send {
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_symlink(&self) -> bool;
}

// pub trait FloppyTempDir:
//     Debug + AsRef<Path> + AsRef<PathBuf> + Send + Sync + Deref<Target = Path>
// {
//     fn path(&self) -> &Path;
// }
