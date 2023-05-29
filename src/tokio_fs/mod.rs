use std::ffi::OsString;
use std::fs::{FileType, Metadata, Permissions};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::SystemTime;

use tokio::fs::{DirBuilder, DirEntry, File, OpenOptions, ReadDir};
use tokio::io::ReadBuf;

use crate::*;

#[derive(Default, Debug)]
pub struct TokioFloppyDisk;

impl TokioFloppyDisk {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl<'a> FloppyDisk<'a> for TokioFloppyDisk {
    type DirBuilder = TokioDirBuilder;
    type DirEntry = TokioDirEntry;
    type File = TokioFile;
    type FileType = TokioFileType;
    type Metadata = TokioMetadata;
    type OpenOptions = TokioOpenOptions;
    type Permissions = TokioPermissions;
    type ReadDir = TokioReadDir;

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

    fn new_dir_builder(&'a self) -> Self::DirBuilder {
        TokioDirBuilder(DirBuilder::new())
    }
}

#[cfg(unix)]
#[async_trait::async_trait]
impl FloppyDiskUnixExt for TokioFloppyDisk {
    async fn chown<P: Into<PathBuf> + Send>(&self, path: P, uid: u32, gid: u32) -> Result<()> {
        let path = path.into();
        tokio::task::spawn_blocking(move || {
            // TODO: Figure out getting rid of
            unsafe {
                use std::os::unix::prelude::OsStrExt;
                libc::chown(
                    path.as_os_str().as_bytes().as_ptr() as *const libc::c_char,
                    uid,
                    gid,
                );
            }
            Ok(())
        })
        .await?
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct TokioMetadata(#[doc(hidden)] Metadata);

#[async_trait::async_trait]
impl<'a> FloppyMetadata<'a, TokioFloppyDisk> for TokioMetadata {
    fn file_type(&self) -> <TokioFloppyDisk as FloppyDisk<'a>>::FileType {
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

    fn permissions(&self) -> <TokioFloppyDisk as FloppyDisk<'a>>::Permissions {
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

#[cfg(unix)]
impl FloppyUnixMetadata for TokioMetadata {
    fn uid(&self) -> Result<u32> {
        use std::os::unix::prelude::MetadataExt;
        Ok(self.0.uid())
    }

    fn gid(&self) -> Result<u32> {
        use std::os::unix::prelude::MetadataExt;
        Ok(self.0.gid())
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct TokioReadDir(#[doc(hidden)] ReadDir);

#[async_trait::async_trait]
impl<'a> FloppyReadDir<'a, TokioFloppyDisk> for TokioReadDir {
    async fn next_entry(
        &mut self,
    ) -> Result<Option<<TokioFloppyDisk as FloppyDisk<'a>>::DirEntry>> {
        self.0
            .next_entry()
            .await
            .map(|entry| entry.map(TokioDirEntry))
    }
}

#[repr(transparent)]
#[derive(Debug)]
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
#[derive(Debug)]
pub struct TokioDirBuilder(#[doc(hidden)] DirBuilder);

#[async_trait::async_trait]
impl FloppyDirBuilder for TokioDirBuilder {
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
#[derive(Debug)]
pub struct TokioDirEntry(#[doc(hidden)] DirEntry);

#[async_trait::async_trait]
impl<'a> FloppyDirEntry<'a, TokioFloppyDisk> for TokioDirEntry {
    fn file_name(&self) -> OsString {
        self.0.file_name()
    }

    async fn file_type(&self) -> Result<<TokioFloppyDisk as FloppyDisk<'a>>::FileType> {
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
#[derive(Debug)]
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

#[derive(Debug)]
pub struct TokioOpenOptions(#[doc(hidden)] OpenOptions);

#[async_trait::async_trait]
impl<'a> FloppyOpenOptions<'a, TokioFloppyDisk> for TokioOpenOptions {
    fn new() -> Self {
        Self(OpenOptions::new())
    }

    fn read(self, read: bool) -> Self {
        let mut oo = self.0;
        oo.read(read);
        Self(oo)
    }

    fn write(self, write: bool) -> Self {
        let mut oo = self.0;
        oo.write(write);
        Self(oo)
    }

    fn append(self, append: bool) -> Self {
        let mut oo = self.0;
        oo.append(append);
        Self(oo)
    }

    fn truncate(self, truncate: bool) -> Self {
        let mut oo = self.0;
        oo.truncate(truncate);
        Self(oo)
    }

    fn create(self, create: bool) -> Self {
        let mut oo = self.0;
        oo.create(create);
        Self(oo)
    }

    fn create_new(self, create_new: bool) -> Self {
        let mut oo = self.0;
        oo.create_new(create_new);
        Self(oo)
    }

    async fn open<P: AsRef<Path> + Send>(
        &self,
        _disk: &'a TokioFloppyDisk,
        path: P,
    ) -> Result<<TokioFloppyDisk as FloppyDisk<'a>>::File> {
        self.0.open(path).await.map(TokioFile)
    }
}

#[derive(Debug)]
#[repr(transparent)]
pub struct TokioFile(#[doc(hidden)] File);

#[async_trait::async_trait]
impl<'a> FloppyFile<'a, TokioFloppyDisk> for TokioFile {
    async fn sync_all(&mut self) -> Result<()> {
        self.0.sync_all().await
    }

    async fn sync_data(&mut self) -> Result<()> {
        self.0.sync_data().await
    }

    async fn set_len(&mut self, size: u64) -> Result<()> {
        self.0.set_len(size).await
    }

    async fn metadata(&self) -> Result<<TokioFloppyDisk as FloppyDisk<'a>>::Metadata> {
        self.0.metadata().await.map(TokioMetadata)
    }

    async fn try_clone(&'a self) -> Result<Box<Self>> {
        self.0
            .try_clone()
            .await
            .map(|file| Box::new(TokioFile(file)))
    }

    async fn set_permissions(
        &self,
        perm: <TokioFloppyDisk as FloppyDisk>::Permissions,
    ) -> Result<()> {
        self.0.set_permissions(perm.0).await
    }

    async fn permissions(&self) -> Result<<TokioFloppyDisk as FloppyDisk<'a>>::Permissions> {
        self.0
            .metadata()
            .await
            .map(|metadata| TokioPermissions(metadata.permissions()))
    }
}

impl AsyncRead for TokioFile {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl AsyncSeek for TokioFile {
    fn start_seek(self: Pin<&mut Self>, position: std::io::SeekFrom) -> std::io::Result<()> {
        Pin::new(&mut self.get_mut().0).start_seek(position)
    }

    fn poll_complete(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<u64>> {
        Pin::new(&mut self.get_mut().0).poll_complete(cx)
    }
}

impl AsyncWrite for TokioFile {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}

// #[derive(Debug)]
// pub struct TokioTempDir {
//     path: PathBuf,
// }

// impl TokioTempDir {
//     async fn new() -> Result<Self> {
//         let mut path = std::env::temp_dir();
//         path.push(format!("peckish-workdir-{}", rand::random::<u64>()));
//         tokio::fs::create_dir_all(&path).await?;

//         Ok(Self { path })
//     }
// }

// impl FloppyTempDir for TokioTempDir {
//     fn path(&self) -> &Path {
//         &self.path
//     }
// }

// impl Drop for TokioTempDir {
//     fn drop(&mut self) {
//         if self.path.exists() {
//             std::fs::remove_dir_all(&self.path).unwrap();
//         }
//     }
// }

// impl AsRef<Path> for TokioTempDir {
//     fn as_ref(&self) -> &Path {
//         &self.path
//     }
// }

// impl AsRef<PathBuf> for TokioTempDir {
//     fn as_ref(&self) -> &PathBuf {
//         &self.path
//     }
// }

// impl std::ops::Deref for TokioTempDir {
//     type Target = Path;

//     fn deref(&self) -> &Self::Target {
//         &self.path
//     }
// }
