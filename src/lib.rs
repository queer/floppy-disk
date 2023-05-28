//! DIY: `#[derive(Clone, Debug)]`
#![deny(unsafe_code)]

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
        FloppyDirBuilder, FloppyDirEntry, FloppyDisk, FloppyFile, FloppyFileType, FloppyMetadata,
        FloppyOpenOptions, FloppyPermissions, FloppyReadDir, FloppyUnixPermissions,
    };

    pub use crate::mem::MemFloppyDisk;
    pub use crate::tokio_fs::TokioFloppyDisk;
}

#[async_trait::async_trait]
pub trait FloppyDisk<'a>: Debug {
    type Metadata: FloppyMetadata;
    type ReadDir: FloppyReadDir;
    type Permissions: FloppyPermissions;
    type DirBuilder: FloppyDirBuilder;
    type OpenOptions: FloppyOpenOptions;
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

    fn new_open_options(&'a self) -> Self::OpenOptions;
}

#[allow(clippy::len_without_is_empty)]
#[async_trait::async_trait]
pub trait FloppyMetadata: Debug {
    type FileType: FloppyFileType;
    type Permissions: FloppyPermissions;

    async fn file_type(&self) -> Self::FileType;
    async fn is_dir(&self) -> bool;
    async fn is_file(&self) -> bool;
    async fn is_symlink(&self) -> bool;
    async fn len(&self) -> u64;
    async fn permissions(&self) -> Self::Permissions;
    async fn modified(&self) -> Result<SystemTime>;
    async fn accessed(&self) -> Result<SystemTime>;
    async fn created(&self) -> Result<SystemTime>;
}

#[async_trait::async_trait]
pub trait FloppyReadDir: Debug {
    type DirEntry: FloppyDirEntry;

    async fn next_entry(&mut self) -> Result<Option<Self::DirEntry>>;
}

pub trait FloppyPermissions: Debug {
    fn readonly(&self) -> bool;
    fn set_readonly(&mut self, readonly: bool);
}

pub trait FloppyUnixPermissions: Debug {
    fn mode(&self) -> u32;
    fn set_mode(&mut self, mode: u32);
    fn from_mode(mode: u32) -> Self;
}

#[async_trait::async_trait]
pub trait FloppyDirBuilder: Debug {
    fn recursive(&mut self, recursive: bool) -> &mut Self;
    async fn create<P: AsRef<Path> + Send>(&self, path: P) -> Result<()>;
    #[cfg(unix)]
    fn mode(&mut self, mode: u32) -> &mut Self;
}

#[async_trait::async_trait]
pub trait FloppyDirEntry: Debug {
    type Metadata: FloppyMetadata;
    type FileType: FloppyFileType;

    fn path(&self) -> PathBuf;
    fn file_name(&self) -> OsString;
    async fn metadata(&self) -> Result<Self::Metadata>;
    async fn file_type(&self) -> Result<Self::FileType>;

    #[cfg(unix)]
    fn ino(&self) -> u64;
}

#[async_trait::async_trait]
pub trait FloppyFile: AsyncRead + AsyncWrite + AsyncSeek + Debug {
    type Metadata: FloppyMetadata;
    type Permissions: FloppyPermissions;

    async fn sync_all(&mut self) -> Result<()>;
    async fn sync_data(&mut self) -> Result<()>;
    async fn set_len(&mut self, size: u64) -> Result<()>;
    async fn metadata(&self) -> Result<Self::Metadata>;
    async fn try_clone(&self) -> Result<Box<Self>>;
    async fn set_permissions(&self, perm: Self::Permissions) -> Result<()>;
    async fn permissions(&self) -> Result<Self::Permissions>;
}

#[async_trait::async_trait]
pub trait FloppyOpenOptions: Debug {
    type File: FloppyFile;

    fn read(&mut self, read: bool) -> &mut Self;
    fn write(&mut self, write: bool) -> &mut Self;
    fn append(&mut self, append: bool) -> &mut Self;
    fn truncate(&mut self, truncate: bool) -> &mut Self;
    fn create(&mut self, create: bool) -> &mut Self;
    fn create_new(&mut self, create_new: bool) -> &mut Self;
    async fn open<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::File>;
}

pub trait FloppyFileType: Debug {
    fn is_dir(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_symlink(&self) -> bool;
}

// pub trait FloppyTempDir:
//     Debug + AsRef<Path> + AsRef<PathBuf> + Send + Sync + Deref<Target = Path>
// {
//     fn path(&self) -> &Path;
// }
