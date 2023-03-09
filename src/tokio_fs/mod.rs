use std::ffi::OsString;
use std::fs::{FileType, Metadata, Permissions};
use std::time::SystemTime;

use tokio::fs::{DirBuilder, DirEntry, ReadDir};

use crate::*;

#[derive(Default)]
pub struct TokioFloppyDisk;

impl TokioFloppyDisk {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FloppyDisk for TokioFloppyDisk {
    type Metadata = TokioMetadata;
    type ReadDir = TokioReadDir;
    type Permissions = TokioPermissions;

    async fn canonicalize<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        tokio::fs::canonicalize(path).await
    }

    async fn copy<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<u64> {
        tokio::fs::copy(from, to).await
    }

    async fn create_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        tokio::fs::create_dir(path).await
    }

    async fn create_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        tokio::fs::create_dir_all(path).await
    }

    async fn hard_link<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()> {
        tokio::fs::hard_link(src, dst).await
    }

    async fn metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        tokio::fs::metadata(path).await.map(TokioMetadata)
    }

    async fn read<P: AsRef<Path> + Send>(&self, path: P) -> Result<Vec<u8>> {
        tokio::fs::read(path).await
    }

    async fn read_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::ReadDir> {
        tokio::fs::read_dir(path).await.map(TokioReadDir)
    }

    async fn read_link<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        tokio::fs::read_link(path).await
    }

    async fn read_to_string<P: AsRef<Path> + Send>(&self, path: P) -> Result<String> {
        tokio::fs::read_to_string(path).await
    }

    async fn remove_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        tokio::fs::remove_dir(path).await
    }

    async fn remove_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        tokio::fs::remove_dir_all(path).await
    }

    async fn remove_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        tokio::fs::remove_file(path).await
    }

    async fn rename<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<()> {
        tokio::fs::rename(from, to).await
    }

    async fn set_permissions<P: AsRef<Path> + Send>(
        &self,
        path: P,
        perm: Self::Permissions,
    ) -> Result<()> {
        tokio::fs::set_permissions(path, perm.0).await
    }

    async fn symlink<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()> {
        tokio::fs::symlink(src, dst).await
    }

    async fn symlink_metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        tokio::fs::symlink_metadata(path).await.map(TokioMetadata)
    }

    async fn try_exists<P: AsRef<Path> + Send>(&self, path: P) -> Result<bool> {
        tokio::fs::try_exists(path).await
    }

    async fn write<P: AsRef<Path> + Send>(
        &self,
        path: P,
        contents: impl AsRef<[u8]> + Send,
    ) -> Result<()> {
        tokio::fs::write(path, contents).await
    }
}

#[repr(transparent)]
pub struct TokioMetadata(#[doc(hidden)] Metadata);

impl FloppyMetadata for TokioMetadata {
    type FileType = TokioFileType;

    type Permissions = TokioPermissions;

    fn file_type(&self) -> Self::FileType {
        TokioFileType(self.0.file_type())
    }

    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    fn is_file(&self) -> bool {
        self.0.is_file()
    }

    fn is_symlink(&self) -> bool {
        self.0.is_symlink()
    }

    fn len(&self) -> u64 {
        self.0.len()
    }

    fn permissions(&self) -> Self::Permissions {
        TokioPermissions(self.0.permissions())
    }

    fn modified(&self) -> Result<SystemTime> {
        self.0.modified()
    }

    fn accessed(&self) -> Result<SystemTime> {
        self.0.accessed()
    }

    fn created(&self) -> Result<SystemTime> {
        self.0.created()
    }
}

#[repr(transparent)]
pub struct TokioReadDir(#[doc(hidden)] ReadDir);

#[async_trait::async_trait]
impl FloppyReadDir for TokioReadDir {
    type DirEntry = TokioDirEntry;

    async fn next_entry(&mut self) -> Result<Option<Self::DirEntry>> {
        self.0
            .next_entry()
            .await
            .map(|entry| entry.map(TokioDirEntry))
    }
}

#[repr(transparent)]
pub struct TokioPermissions(#[doc(hidden)] Permissions);

impl FloppyPermissions for TokioPermissions {
    fn readonly(&self) -> bool {
        self.0.readonly()
    }

    fn set_readonly(&mut self, readonly: bool) {
        self.0.set_readonly(readonly)
    }
}

#[repr(transparent)]
pub struct TokioDirBuilder(DirBuilder);

#[async_trait::async_trait]
impl FloppyDirBuilder for TokioDirBuilder {
    fn new() -> Self {
        Self(DirBuilder::new())
    }

    fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.0.recursive(recursive);
        self
    }

    async fn create<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.0.create(path).await
    }

    fn mode(&mut self, mode: u32) -> &mut Self {
        self.0.mode(mode);
        self
    }
}

#[repr(transparent)]
pub struct TokioDirEntry(#[doc(hidden)] DirEntry);

#[async_trait::async_trait]
impl FloppyDirEntry for TokioDirEntry {
    type FileType = TokioFileType;
    type Metadata = TokioMetadata;

    fn file_name(&self) -> OsString {
        self.0.file_name()
    }

    async fn file_type(&self) -> Result<Self::FileType> {
        self.0.file_type().await.map(TokioFileType)
    }

    async fn metadata(&self) -> Result<TokioMetadata> {
        self.0.metadata().await.map(TokioMetadata)
    }

    fn path(&self) -> PathBuf {
        self.0.path()
    }

    #[cfg(unix)]
    fn ino(&self) -> u64 {
        self.0.ino()
    }
}

#[repr(transparent)]
pub struct TokioFileType(#[doc(hidden)] FileType);

impl FloppyFileType for TokioFileType {
    fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    fn is_file(&self) -> bool {
        self.0.is_file()
    }

    fn is_symlink(&self) -> bool {
        self.0.is_symlink()
    }
}
