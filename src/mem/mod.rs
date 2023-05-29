use std::ffi::OsString;
use std::io::{Read, Result, Seek, Write};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;

use derivative::Derivative;
use futures::{Future, TryStreamExt};
use rsfs_tokio::unix_ext::{GenFSExt, PermissionsExt};
use rsfs_tokio::{DirEntry, File, FileType, GenFS, Metadata, OpenOptions};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
pub type InMemoryUnixFS = rsfs_tokio::mem::unix::FS;

// TODO: DirBuilder, OpenOptions
use crate::{
    FloppyDirBuilder, FloppyDirEntry, FloppyDisk, FloppyDiskUnixExt, FloppyFile, FloppyFileType,
    FloppyMetadata, FloppyOpenOptions, FloppyPermissions, FloppyReadDir, FloppyUnixMetadata,
    FloppyUnixPermissions,
};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFloppyDisk {
    fs: InMemoryUnixFS,
}

impl MemFloppyDisk {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            fs: InMemoryUnixFS::new(),
        }
    }
}

#[async_trait::async_trait]
impl<'a> FloppyDisk<'a> for MemFloppyDisk {
    type DirBuilder = MemDirBuilder<'a>;
    type DirEntry = MemDirEntry;
    type File = MemFile;
    type FileType = MemFileType;
    type Metadata = MemMetadata;
    type OpenOptions = MemOpenOptions;
    type Permissions = MemPermissions;
    type ReadDir = MemReadDir;

    async fn canonicalize<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        self.fs.canonicalize(path).await
    }

    async fn copy<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<u64> {
        self.fs.copy(from, to).await
    }

    async fn create_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.fs.create_dir(path).await
    }

    async fn create_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.fs.create_dir_all(path).await
    }

    async fn hard_link<P: AsRef<Path> + Send>(&self, _src: P, _dst: P) -> Result<()> {
        unimplemented!("hard links are not yet supported")
    }

    async fn metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        let metadata = self.fs.metadata(path).await?;
        Ok(Self::Metadata { metadata })
    }

    async fn read<P: AsRef<Path> + Send>(&self, path: P) -> Result<Vec<u8>> {
        let mut file = self.fs.open_file(path).await?;
        let file_len = file.metadata().await?.len() as usize;
        let mut buffer = vec![0u8; file_len];
        let read = file.read(&mut buffer).await?;
        debug_assert!(read <= buffer.len());
        Ok(buffer)
    }

    async fn read_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::ReadDir> {
        self.fs.read_dir(path).await.map(MemReadDir::new)
    }

    async fn read_link<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        self.fs.read_link(path).await
    }

    async fn read_to_string<P: AsRef<Path> + Send>(&self, path: P) -> Result<String> {
        let mut file = self.fs.open_file(path).await?;
        let file_len = file.metadata().await?.len() as usize;
        let mut buffer = String::with_capacity(file_len);
        file.read_to_string(&mut buffer).await?;
        Ok(buffer)
    }

    async fn remove_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.fs.remove_dir(path).await
    }

    async fn remove_dir_all<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.fs.remove_dir_all(path).await
    }

    async fn remove_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        self.fs.remove_file(path).await
    }

    async fn rename<P: AsRef<Path> + Send>(&self, from: P, to: P) -> Result<()> {
        self.fs.rename(from, to).await
    }

    async fn set_permissions<P: AsRef<Path> + Send>(
        &mut self,
        path: P,
        perm: Self::Permissions,
    ) -> Result<()> {
        self.fs
            .set_permissions(path, rsfs_tokio::mem::Permissions::from_mode(perm.mode()))
            .await
    }

    async fn symlink<P: AsRef<Path> + Send>(&self, src: P, dst: P) -> Result<()> {
        self.fs.symlink(src, dst).await
    }

    async fn symlink_metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        self.fs
            .symlink_metadata(path)
            .await
            .map(|metadata| Self::Metadata { metadata })
    }

    async fn try_exists<P: AsRef<Path> + Send>(&self, path: P) -> Result<bool> {
        Ok(self.fs.metadata(path).await.is_ok())
    }

    async fn write<P: AsRef<Path> + Send>(
        &self,
        path: P,
        contents: impl AsRef<[u8]> + Send,
    ) -> Result<()> {
        let mut file = self.fs.create_file(path).await?;
        let contents = contents.as_ref();
        file.write_all(contents).await?;
        Ok(())
    }

    fn new_dir_builder(&'a self) -> Self::DirBuilder {
        MemDirBuilder {
            fs: self,
            recursive: false,
            #[cfg(unix)]
            mode: 0o777,
        }
    }
}

#[async_trait::async_trait]
impl FloppyDiskUnixExt for MemFloppyDisk {
    async fn chown<P: Into<PathBuf> + Send>(&self, path: P, uid: u32, gid: u32) -> Result<()> {
        self.fs.set_ownership(path.into(), uid, gid).await
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFile {
    file: rsfs_tokio::mem::unix::File,
}

#[async_trait::async_trait]
impl<'a> FloppyFile<'a, MemFloppyDisk> for MemFile {
    async fn sync_all(&mut self) -> Result<()> {
        Ok(())
    }

    async fn sync_data(&mut self) -> Result<()> {
        Ok(())
    }

    async fn set_len(&mut self, size: u64) -> Result<()> {
        self.file.set_len(size).await
    }

    async fn metadata(&self) -> Result<<MemFloppyDisk as FloppyDisk>::Metadata> {
        Ok(MemMetadata {
            metadata: self.file.metadata().await?,
        })
    }

    async fn try_clone(&'a self) -> Result<Box<Self>> {
        Ok(Box::new(Self {
            file: self.file.try_clone().await?,
        }))
    }

    async fn set_permissions(
        &self,
        perm: <MemFloppyDisk as FloppyDisk>::Permissions,
    ) -> Result<()> {
        self.file
            .set_permissions(rsfs_tokio::mem::Permissions::from_mode(perm.mode()))
            .await
    }

    async fn permissions(&self) -> Result<<MemFloppyDisk as FloppyDisk>::Permissions> {
        Ok(MemPermissions {
            mode: self.file.metadata().await?.permissions().mode(),
        })
    }
}

impl AsyncSeek for MemFile {
    fn start_seek(mut self: Pin<&mut Self>, position: std::io::SeekFrom) -> Result<()> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.start_seek(position)
    }

    fn poll_complete(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<u64>> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.poll_complete(cx)
    }
}

impl AsyncRead for MemFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<Result<()>> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.poll_read(cx, buf)
    }
}

impl AsyncWrite for MemFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize>> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<()>> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<()>> {
        let mut this = self.as_mut();
        let file = Pin::new(&mut this.file);
        file.poll_shutdown(cx)
    }
}

impl Read for MemFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        run_here_outside_of_tokio_context(async { self.file.read(buf).await })
    }
}

impl Write for MemFile {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        run_here_outside_of_tokio_context(async { self.file.write(buf).await })
    }

    fn flush(&mut self) -> Result<()> {
        run_here_outside_of_tokio_context(async { self.file.flush().await })
    }
}

impl Seek for MemFile {
    fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
        run_here_outside_of_tokio_context(async { self.file.seek(pos).await })
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

impl FloppyUnixPermissions for MemPermissions {
    fn mode(&self) -> u32 {
        self.mode
    }

    fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
    }

    fn from_mode(mode: u32) -> Self {
        Self { mode }
    }
}

#[derive(Debug)]
pub struct MemMetadata {
    metadata: rsfs_tokio::mem::unix::Metadata,
}

#[async_trait::async_trait]
impl<'a> FloppyMetadata<'a, MemFloppyDisk> for MemMetadata {
    fn file_type(&self) -> <MemFloppyDisk as FloppyDisk>::FileType {
        MemFileType(self.metadata.file_type())
    }

    fn is_dir(&self) -> bool {
        self.metadata.is_dir()
    }

    fn is_file(&self) -> bool {
        self.metadata.is_file()
    }

    fn is_symlink(&self) -> bool {
        self.metadata.file_type().is_symlink()
    }

    fn len(&self) -> u64 {
        self.metadata.len()
    }

    fn permissions(&self) -> <MemFloppyDisk as FloppyDisk>::Permissions {
        MemPermissions {
            mode: self.metadata.permissions().mode(),
        }
    }

    fn modified(&self) -> Result<SystemTime> {
        self.metadata.modified()
    }

    fn accessed(&self) -> Result<SystemTime> {
        self.metadata.accessed()
    }

    fn created(&self) -> Result<SystemTime> {
        self.metadata.created()
    }
}

impl FloppyUnixMetadata for MemMetadata {
    fn uid(&self) -> Result<u32> {
        self.metadata.uid()
    }

    fn gid(&self) -> Result<u32> {
        self.metadata.gid()
    }
}

#[derive(Debug)]
pub struct MemFileType(#[doc(hidden)] rsfs_tokio::mem::unix::FileType);

impl FloppyFileType for MemFileType {
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
pub struct MemReadDir {
    read_dir: rsfs_tokio::mem::unix::ReadDir,
}

impl MemReadDir {
    fn new(read_dir: rsfs_tokio::mem::unix::ReadDir) -> Self {
        Self { read_dir }
    }
}

#[async_trait::async_trait]
impl<'a> FloppyReadDir<'a, MemFloppyDisk> for MemReadDir {
    async fn next_entry(&mut self) -> Result<Option<<MemFloppyDisk as FloppyDisk>::DirEntry>> {
        match self.read_dir.try_next().await {
            Ok(Some(Some(entry))) => Ok(Some(MemDirEntry { entry })),
            Ok(Some(None)) => Ok(None),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug)]
pub struct MemDirEntry {
    entry: rsfs_tokio::mem::unix::DirEntry,
}

#[async_trait::async_trait]
impl<'a> FloppyDirEntry<'a, MemFloppyDisk> for MemDirEntry {
    fn path(&self) -> PathBuf {
        self.entry.path()
    }
    fn file_name(&self) -> OsString {
        self.entry.file_name()
    }
    async fn metadata(&self) -> Result<<MemFloppyDisk as FloppyDisk>::Metadata> {
        Ok(MemMetadata {
            metadata: self.entry.metadata().await?,
        })
    }
    async fn file_type(&self) -> Result<<MemFloppyDisk as FloppyDisk>::FileType> {
        Ok(MemFileType(self.entry.file_type().await?))
    }

    #[cfg(unix)]
    fn ino(&self) -> u64 {
        unimplemented!("not currently supported")
    }
}

#[derive(Debug)]
pub struct MemDirBuilder<'a> {
    fs: &'a MemFloppyDisk,
    recursive: bool,
    #[cfg(unix)]
    mode: u32,
}

#[async_trait::async_trait]
impl FloppyDirBuilder for MemDirBuilder<'_> {
    fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    async fn create<P: AsRef<Path> + Send>(&self, path: P) -> Result<()> {
        if self.recursive {
            self.fs.create_dir_all(path).await
        } else {
            self.fs.create_dir(path).await
        }
    }

    #[cfg(unix)]
    fn mode(&mut self, mode: u32) -> &mut Self {
        self.mode = mode;
        self
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MemOpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

#[async_trait::async_trait]
impl<'a> FloppyOpenOptions<'a, MemFloppyDisk> for MemOpenOptions {
    fn new() -> Self {
        Self {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
        }
    }

    fn read(mut self, read: bool) -> Self {
        self.read = read;
        self
    }

    fn write(mut self, write: bool) -> Self {
        self.write = write;
        self
    }

    fn append(mut self, append: bool) -> Self {
        self.append = append;
        self
    }

    fn truncate(mut self, truncate: bool) -> Self {
        self.truncate = truncate;
        self
    }

    fn create(mut self, create: bool) -> Self {
        self.create = create;
        self
    }

    fn create_new(mut self, create_new: bool) -> Self {
        self.create_new = create_new;
        self
    }

    async fn open<P: AsRef<Path> + Send>(
        &self,
        disk: &'a MemFloppyDisk,
        path: P,
    ) -> Result<<MemFloppyDisk as FloppyDisk<'a>>::File> {
        let mut options = disk.fs.new_openopts();
        options.read(self.read);
        options.write(self.write);
        options.append(self.append);
        options.truncate(self.truncate);
        options.create(self.create);
        options.create_new(self.create_new);
        let file = options.open(path).await?;
        Ok(MemFile { file })
    }
}

#[allow(unused)]
fn run_here<F: Future>(fut: F) -> F::Output {
    // TODO: This is evil
    // Adapted from https://stackoverflow.com/questions/66035290
    let handle = tokio::runtime::Handle::try_current().unwrap();
    let _guard = handle.enter();
    futures::executor::block_on(fut)
}

#[allow(unused)]
fn run_here_outside_of_tokio_context<F: Future>(fut: F) -> F::Output {
    // TODO: This is slightly less-evil than the previous one but still pretty bad
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(fut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::io::Result;

    #[tokio::test]
    async fn test_mem_floppy_disk() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        assert_eq!("asdf", fs.read_to_string("/test.txt").await?);

        Ok(())
    }

    // FIXME
    // #[tokio::test]
    // async fn test_canonicalize() -> Result<()> {
    //     let fs = MemFloppyDisk::new();

    //     assert_eq!(PathBuf::from("/"), fs.canonicalize("/").await?);
    //     assert_eq!(PathBuf::from("/"), fs.canonicalize("/.").await?);
    //     assert_eq!(PathBuf::from("/"), fs.canonicalize("/..").await?);
    //     assert_eq!(PathBuf::from("/"), fs.canonicalize("/../..").await?);
    //     assert_eq!(PathBuf::from("a"), fs.canonicalize("a").await?);
    //     assert_eq!(PathBuf::from("a"), fs.canonicalize("a/.").await?);
    //     assert_eq!(PathBuf::from("/a"), fs.canonicalize("/a/.").await?);
    //     assert_eq!(PathBuf::from("/a"), fs.canonicalize("/a/../a").await?);
    //     assert_eq!(
    //         PathBuf::from("/"),
    //         fs.canonicalize("/usr/bin/../../../../../../..").await?
    //     );

    //     Ok(())
    // }

    #[tokio::test]
    async fn test_copy() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.copy("/test.txt", "/test2.txt").await?;
        assert_eq!("asdf", fs.read_to_string("/test2.txt").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_dir() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.create_dir("/test").await?;
        let metadata = fs.metadata("/test").await?;
        assert!(metadata.is_dir());

        Ok(())
    }

    #[tokio::test]
    async fn test_create_dir_all() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.create_dir_all("/test/a/b/c").await?;
        let metadata = fs.metadata("/test/a/b/c").await?;
        assert!(metadata.is_dir());

        Ok(())
    }

    // #[tokio::test]
    // async fn test_hard_link() -> Result<()> {
    //     let mut fs = MemFloppyDisk::new();
    //     fs.write("/test.txt", "asdf").await?;
    //     fs.hard_link("/test.txt", "/test2.txt").await?;
    //     assert_eq!("asdf", fs.read_to_string("/test2.txt").await?);

    //     Ok(())
    // }

    #[tokio::test]
    async fn test_metadata() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let metadata = fs.metadata("/test.txt").await?;
        assert!(metadata.is_file());
        assert_eq!(4, metadata.len());

        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let buf = fs.read("/test.txt").await?;
        assert_eq!(b"asdf", buf.as_slice());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_dir() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.create_dir("/test").await?;
        let mut entries = fs.read_dir("/").await?;
        let entry = entries.next_entry().await?;
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!("test", entry.file_name().to_str().unwrap());
        assert!(entry.file_type().await?.is_dir());

        let entry = entries.next_entry().await?;
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!("test.txt", entry.file_name().to_str().unwrap());
        assert!(entry.file_type().await?.is_file());

        assert!(entries.next_entry().await?.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_link() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let s = fs.read_link("/test2.txt").await?;
        assert_eq!(PathBuf::from("/test.txt"), s);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_to_string() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let s = fs.read_to_string("/test.txt").await?;
        assert_eq!("asdf", s);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_dir() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.create_dir("/test").await?;
        fs.remove_dir("/test").await?;
        assert!(fs.metadata("/test").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_dir_all() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.create_dir_all("/test/a/b/c").await?;
        fs.remove_dir_all("/test").await?;
        assert!(fs.metadata("/test").await.is_err());
        assert!(fs.metadata("/test/a").await.is_err());
        assert!(fs.metadata("/test/a/b").await.is_err());
        assert!(fs.metadata("/test/a/b/c").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_file() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.remove_file("/test.txt").await?;
        assert!(fs.metadata("/test.txt").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_rename() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.rename("/test.txt", "/test2.txt").await?;
        assert!(fs.metadata("/test.txt").await.is_err());
        assert_eq!("asdf", fs.read_to_string("/test2.txt").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_set_permissions() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.set_permissions("/test.txt", MemPermissions::from_mode(0o777))
            .await?;
        let metadata = fs.metadata("/test.txt").await?;
        assert_eq!(0o777, metadata.permissions().mode());

        Ok(())
    }

    #[tokio::test]
    async fn test_symlink_metadata() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let metadata = fs.symlink_metadata("/test2.txt").await?;
        assert!(metadata.is_symlink());

        Ok(())
    }

    #[tokio::test]
    async fn test_symlink() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let s = fs.read_link("/test2.txt").await?;
        assert_eq!(PathBuf::from("/test.txt"), s);

        Ok(())
    }

    #[tokio::test]
    async fn test_write() -> Result<()> {
        let fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        assert_eq!("asdf", fs.read_to_string("/test.txt").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_dir_builder() -> Result<()> {
        let fs = MemFloppyDisk::new();
        {
            let mut builder = fs.new_dir_builder();
            builder.recursive(true);
            builder.create("/test/a/b/c").await?;

            assert!(fs.metadata("/test").await?.is_dir());
            assert!(fs.metadata("/test/a").await?.is_dir());
            assert!(fs.metadata("/test/a/b").await?.is_dir());
            assert!(fs.metadata("/test/a/b/c").await?.is_dir());
        }

        {
            let mut builder = fs.new_dir_builder();
            builder.recursive(false);
            let res = builder.create("/test2/a/b/c").await;
            assert!(res.is_err());
        }

        Ok(())
    }
}
