use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Result;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

use derivative::Derivative;
use futures::Future;
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite};
use tokio::sync::Mutex;
use tokio::sync::RwLockWriteGuard;

use crate::FloppyDirEntry;
use crate::FloppyDisk;
use crate::FloppyReadDir;
use crate::FloppyTempDir;
use crate::FloppyUnixPermissions;
use crate::{FloppyFile, FloppyFileType, FloppyMetadata, FloppyPermissions};

use self::inode::{Inode, InodeType};

type TokioRwLock<T> = tokio::sync::RwLock<T>;
type MemTree = BTreeMap<PathBuf, Arc<TokioRwLock<Inode>>>;

mod inode;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFloppyDisk<'a> {
    inode_serial: u64,
    fs: tokio::sync::RwLock<MemTree>,
    #[derivative(Debug = "ignore")]
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl MemFloppyDisk<'_> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let fs = {
            let mut fs = BTreeMap::new();
            let root_node = Inode::new_dir(0, PathBuf::from("/"));
            fs.insert(PathBuf::from("/"), Arc::new(TokioRwLock::new(root_node)));
            fs
        };
        Self {
            inode_serial: 1,
            fs: tokio::sync::RwLock::new(fs),
            _phantom: std::marker::PhantomData,
        }
    }

    async fn find_lowest_non_existing_parent(&self, input: &Path) -> Result<Option<PathBuf>> {
        if let Some(parent) = input.parent() {
            let mut path = PathBuf::new();
            let mut found = false;

            for component in parent.components() {
                path.push(component);

                if !self.fs.read().await.contains_key(&path) {
                    found = true;
                    break;
                }
            }

            if found {
                Ok(Some(path))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn make_sure_parent_exists(&self, path: &Path) -> Result<()> {
        // If a parent doesn't exist, fail
        if let Some(non_existing_parent) = self.find_lowest_non_existing_parent(path).await? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("make_sure_parent_exists: {non_existing_parent:?} does not exist"),
            ));
        }

        Ok(())
    }

    async fn make_sure_path_exists(&self, path: &Path) -> Result<()> {
        let fs = self.fs.read().await;

        // If a parent doesn't exist, fail
        if let Some(non_existing_parent) = self.find_lowest_non_existing_parent(path).await? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("make_sure_path_exists: {non_existing_parent:?} does not exist"),
            ));
        }

        // If the given path isn't in the tree, fail
        if !fs.contains_key(path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("make_sure_path_exists: {path:?} does not exist"),
            ));
        }

        Ok(())
    }

    async fn make_sure_path_doesnt_exist(&self, path: &Path) -> Result<()> {
        let fs = self.fs.read().await;

        // If a parent doesn't exist, fail
        if let Some(non_existing_parent) = self.find_lowest_non_existing_parent(path).await? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("make_sure_path_doesnt_exist: {non_existing_parent:?} does not exist"),
            ));
        }

        // If the given path is in the tree, fail
        if fs.contains_key(path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("make_sure_path_doesnt_exist: {path:?} already exists"),
            ));
        }

        Ok(())
    }

    fn get_next_inode_serial(&mut self) -> u64 {
        self.inode_serial += 1;
        self.inode_serial
    }

    fn insert_inode<P: Into<PathBuf>>(
        &self,
        fs: &mut RwLockWriteGuard<'_, MemTree>,
        path: P,
        inode: Inode,
    ) -> Result<()> {
        fs.insert(path.into(), Arc::new(tokio::sync::RwLock::new(inode)));

        Ok(())
    }
}

#[async_trait::async_trait]
impl<'a> FloppyDisk<'a> for MemFloppyDisk<'a> {
    type Metadata = MemMetadata;
    type ReadDir = MemReadDir;
    type Permissions = MemPermissions;
    type TempDir = MemTempDir<'a>;

    async fn canonicalize<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        // Resolve all .. and symlinks
        // If . or .. is the very first component of the path, we return an error as there is no concept of pwd currently
        // Otherwise, we treat it as a parent dir
        // TODO: Support pwd
        let path = path.as_ref();
        let mut out = PathBuf::new();

        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) => {
                    out.push(component);
                }
                std::path::Component::RootDir => {
                    out.push(component);
                }
                std::path::Component::CurDir => {
                    // Ignore
                }
                std::path::Component::ParentDir => {
                    if out.components().count() > 0 {
                        out.pop();
                    }
                }
                std::path::Component::Normal(_) => {
                    out.push(component);
                }
            }
        }

        // Resolve symlinks
        let fs = self.fs.read().await;
        if let Some(inode) = fs.get(&out) {
            let mut inode = inode.read().await;

            while *inode.kind() == InodeType::Symlink {
                assert!(inode.symlink_target().is_some());
                let target = inode.symlink_target().as_ref().unwrap().clone();
                out = target;
                inode = fs.get(&out).unwrap().read().await;
            }
        }

        Ok(out)
    }

    async fn copy<P: AsRef<Path> + Send>(&mut self, from: P, to: P) -> Result<u64> {
        let from = from.as_ref();
        let to = to.as_ref();
        println!("make sure paths exist");
        self.make_sure_path_exists(from).await?;

        let next_inode = self.get_next_inode_serial();
        let mut fs = self.fs.write().await;
        let to_inode = {
            println!("acquire write lock");
            println!("lock get!");

            // Remove old inode
            println!("remove old inode");
            fs.remove(to);

            // Clone new inode from incoming path
            println!("clone new inode");
            let from_inode = fs.get(from).unwrap().read().await;
            let mut to_inode = Inode::new_file(next_inode, to, from_inode.buffer().clone());
            to_inode.set_permissions(from_inode.permissions());
            to_inode
        };

        // Insert new inode
        println!("insert new inode");
        let out = to_inode.len();
        self.insert_inode(&mut fs, to, to_inode)?;

        Ok(out)
    }

    async fn create_dir<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        self.make_sure_path_doesnt_exist(path).await?;

        let next_inode = self.get_next_inode_serial();

        let inode = Inode::new_dir(next_inode, path);

        let mut fs = self.fs.write().await;
        self.insert_inode(&mut fs, path, inode)?;

        Ok(())
    }

    async fn create_dir_all<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();

        // While parents don't exist, create them
        while let Some(non_existing_parent) = self.find_lowest_non_existing_parent(path).await? {
            self.create_dir(non_existing_parent).await?;
        }

        // Create the final path
        self.create_dir(path).await
    }

    async fn hard_link<P: AsRef<Path> + Send>(&mut self, _src: P, _dst: P) -> Result<()> {
        unimplemented!("hard links are not yet supported")
    }

    async fn metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let inode = fs.get(path).unwrap().clone();

        Ok(MemMetadata { inode })
    }

    async fn read<P: AsRef<Path> + Send>(&self, path: P) -> Result<Vec<u8>> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let inode = fs.get(path).unwrap().read().await;

        if *inode.kind() != InodeType::File {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{path:?} is not a file"),
            ));
        }

        Ok(inode.buffer_view().to_vec())
    }

    async fn read_dir<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::ReadDir> {
        // Find all paths in the fs that start with the given path and have exactly one component after the given path
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let mut out = Vec::new();

        for (key, value) in fs.iter() {
            if key.starts_with(path) && key.components().count() == path.components().count() + 1 {
                out.push(value.clone());
            }
        }

        Ok(MemReadDir::new(out))
    }

    async fn read_link<P: AsRef<Path> + Send>(&self, path: P) -> Result<PathBuf> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let inode = fs.get(path).unwrap().read().await;

        if *inode.kind() != InodeType::Symlink {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{path:?} is not a symlink"),
            ));
        }

        assert!(inode.symlink_target().is_some());

        Ok(inode.symlink_target().as_ref().unwrap().clone())
    }

    async fn read_to_string<P: AsRef<Path> + Send>(&self, path: P) -> Result<String> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let inode = fs.get(path).unwrap().read().await;

        let out = std::str::from_utf8(inode.buffer_view())
            .unwrap()
            .to_string();

        Ok(out)
    }

    async fn remove_dir<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        if self.read_dir(path).await?.inodes.is_empty() {
            let mut fs = self.fs.write().await;
            fs.remove(path).unwrap();

            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{path:?} is not empty"),
            ))
        }
    }

    async fn remove_dir_all<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<()> {
        // Recursively remove all children in dir, depth-first
        // Finally, remove dir
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let mut fs = self.fs.write().await;
        let mut to_remove = Vec::new();

        for (key, _) in fs.iter() {
            if key.starts_with(path) {
                to_remove.push(key.clone());
            }
        }

        for key in to_remove {
            fs.remove(&key).unwrap();
        }

        Ok(())
    }

    async fn remove_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let mut fs = self.fs.write().await;
        fs.remove(path).unwrap();

        Ok(())
    }

    async fn rename<P: AsRef<Path> + Send>(&mut self, from: P, to: P) -> Result<()> {
        let from = from.as_ref();
        let to = to.as_ref();
        self.make_sure_path_exists(from).await?;
        self.make_sure_path_doesnt_exist(to).await?;

        let next_inode = self.get_next_inode_serial();
        let mut fs = self.fs.write().await;
        let old_inode = fs.remove(from).unwrap();
        let mut new_inode = old_inode.read().await.clone();
        new_inode.set_serial(next_inode);

        self.insert_inode(&mut fs, to, new_inode)?;

        Ok(())
    }

    async fn set_permissions<P: AsRef<Path> + Send>(
        &mut self,
        path: P,
        perm: Self::Permissions,
    ) -> Result<()> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.write().await;
        let inode = fs.get(path).unwrap();
        let mut inode = inode.write().await;
        inode.set_permissions(perm);

        Ok(())
    }

    async fn symlink<P: AsRef<Path> + Send>(&mut self, src: P, dst: P) -> Result<()> {
        let src = src.as_ref();
        let dst = dst.as_ref();
        self.make_sure_path_exists(src).await?;
        self.make_sure_path_doesnt_exist(dst).await?;

        let next_inode = self.get_next_inode_serial();

        let mut fs = self.fs.write().await;
        self.insert_inode(&mut fs, dst, Inode::new_symlink(next_inode, dst, src))?;

        Ok(())
    }

    async fn symlink_metadata<P: AsRef<Path> + Send>(&self, path: P) -> Result<Self::Metadata> {
        let path = path.as_ref();
        self.make_sure_path_exists(path).await?;

        let fs = self.fs.read().await;
        let inode = fs.get(path).unwrap().read().await;

        if *inode.kind() == InodeType::Symlink {
            if let Some(target) = inode.symlink_target() {
                if self.make_sure_path_exists(target).await.is_err() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "symlink {:?} points to non-existing target {:?}",
                            path, target
                        ),
                    ));
                }

                let target_inode = fs.get(target).unwrap().clone();
                Ok(MemMetadata {
                    inode: target_inode,
                })
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("symlink {:?} has no target", path),
                ))
            }
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("{:?} is not a symlink", path),
            ));
        }
    }

    async fn try_exists<P: AsRef<Path> + Send>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        let fs = self.fs.read().await;
        let mut path = Some(path.to_path_buf());
        while let Some(current_path) = path {
            if let Some(inode) = fs.get(&current_path.to_path_buf()) {
                let inode = inode.read().await;
                if *inode.kind() == InodeType::Symlink {
                    if !inode.symlink_target().is_some() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("symlink {:?} has no target", current_path),
                        ));
                    }

                    let symlink_target = inode.symlink_target().clone();
                    let symlink_target = symlink_target.unwrap();
                    path = Some(symlink_target);
                } else {
                    return Ok(true);
                }
            } else {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn write<P: AsRef<Path> + Send>(
        &mut self,
        path: P,
        contents: impl AsRef<[u8]> + Send,
    ) -> Result<()> {
        let path = path.as_ref();
        self.make_sure_parent_exists(path).await?;

        let next_inode = self.get_next_inode_serial();

        let mut fs = self.fs.write().await;
        let inode = Inode::new_file(next_inode, path, contents.as_ref().to_vec());
        self.insert_inode(&mut fs, path, inode)?;

        Ok(())
    }

    async fn create_tmp_dir(&mut self) -> Result<Self::TempDir>
    where
        'life0: 'a,
    {
        MemTempDir::new(Arc::new(Mutex::new(self))).await
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct MemFile {
    inode: Arc<TokioRwLock<Inode>>,
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
        let mut inode = self.inode.write().await;
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
        let mut inode = self.inode.write().await;
        inode.set_permissions(perm);
        Ok(())
    }

    async fn permissions(&self) -> Result<Self::Permissions> {
        let inode = self.inode.read().await;
        Ok(inode.permissions())
    }
}

fn run_here<F: Future>(fut: F) -> F::Output {
    // TODO: This is evil
    // Adapted from https://stackoverflow.com/questions/66035290
    let handle = tokio::runtime::Handle::try_current().unwrap();
    let _guard = handle.enter();
    futures::executor::block_on(fut)
}

impl AsyncSeek for MemFile {
    fn start_seek(self: Pin<&mut Self>, position: std::io::SeekFrom) -> Result<()> {
        let mut this = self.get_mut();

        match position {
            std::io::SeekFrom::Start(pos) => this.position = pos,
            std::io::SeekFrom::End(pos) => {
                run_here(async {
                    let inode = this.inode.read().await;
                    this.position = (inode.len() as i64 + pos) as u64;

                    if this.position > inode.len() {
                        this.position = inode.len();
                    }
                });
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
        run_here(async {
            let mut this = self.get_mut();
            let inode = this.inode.read().await;
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
        })
    }
}

impl AsyncWrite for MemFile {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize>> {
        run_here(async {
            let this = self.get_mut();
            let mut inode = this.inode.write().await;

            let position = this.position;
            let len = buf.len();

            let buffer = inode.buffer_mut();
            buffer.resize(position as usize + len, 0);
            buffer[position as usize..position as usize + len].copy_from_slice(buf);

            this.position += len as u64;

            std::task::Poll::Ready(Ok(len))
        })
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
    inode: Arc<TokioRwLock<Inode>>,
}

#[async_trait::async_trait]
impl FloppyMetadata for MemMetadata {
    type FileType = MemFileType;

    type Permissions = MemPermissions;

    async fn file_type(&self) -> Self::FileType {
        let inode = self.inode.read().await;
        match inode.kind() {
            InodeType::File => MemFileType(InodeType::File),
            InodeType::Dir => MemFileType(InodeType::Dir),
            InodeType::Symlink => MemFileType(InodeType::Symlink),
        }
    }

    async fn is_dir(&self) -> bool {
        let inode = self.inode.read().await;
        *inode.kind() == InodeType::Dir
    }

    async fn is_file(&self) -> bool {
        let inode = self.inode.read().await;
        *inode.kind() == InodeType::File
    }

    async fn is_symlink(&self) -> bool {
        let inode = self.inode.read().await;
        *inode.kind() == InodeType::Symlink
    }

    async fn len(&self) -> u64 {
        let inode = self.inode.read().await;
        inode.len()
    }

    async fn permissions(&self) -> Self::Permissions {
        let inode = self.inode.read().await;
        inode.mode().clone()
    }

    async fn modified(&self) -> Result<SystemTime> {
        let inode = self.inode.read().await;
        Ok(*inode.mtime())
    }

    async fn accessed(&self) -> Result<SystemTime> {
        let inode = self.inode.read().await;
        Ok(*inode.atime())
    }

    async fn created(&self) -> Result<SystemTime> {
        let inode = self.inode.read().await;
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

#[derive(Debug)]
pub struct MemReadDir {
    inodes: Vec<Arc<TokioRwLock<Inode>>>,
    idx: usize,
}

impl MemReadDir {
    fn new(inodes: Vec<Arc<TokioRwLock<Inode>>>) -> Self {
        Self { inodes, idx: 0 }
    }
}

#[async_trait::async_trait]
impl FloppyReadDir for MemReadDir {
    type DirEntry = MemDirEntry;

    async fn next_entry(&mut self) -> Result<Option<Self::DirEntry>> {
        if self.idx >= self.inodes.len() {
            return Ok(None);
        }

        let inode = self.inodes[self.idx].clone();
        self.idx += 1;

        Ok(Some(MemDirEntry { inode }))
    }
}

#[derive(Debug)]
pub struct MemDirEntry {
    inode: Arc<TokioRwLock<Inode>>,
}

#[async_trait::async_trait]
impl FloppyDirEntry for MemDirEntry {
    type Metadata = MemMetadata;
    type FileType = MemFileType;

    fn path(&self) -> PathBuf {
        run_here(async {
            let inode = self.inode.read().await;
            inode.path().clone()
        })
    }
    fn file_name(&self) -> OsString {
        run_here(async {
            let inode = self.inode.read().await;
            inode.path().file_name().unwrap().to_os_string()
        })
    }
    async fn metadata(&self) -> Result<Self::Metadata> {
        Ok(MemMetadata {
            inode: self.inode.clone(),
        })
    }
    async fn file_type(&self) -> Result<Self::FileType> {
        let inode = self.inode.read().await;
        Ok(MemFileType(*inode.kind()))
    }

    #[cfg(unix)]
    fn ino(&self) -> u64 {
        run_here(async {
            let inode = self.inode.read().await;
            *inode.serial()
        })
    }
}

#[derive(Debug)]
pub struct MemTempDir<'a> {
    path: PathBuf,
    fs: Arc<Mutex<&'a mut MemFloppyDisk<'a>>>,
}

impl<'a> MemTempDir<'a> {
    async fn new(fs: Arc<Mutex<&'a mut MemFloppyDisk<'a>>>) -> Result<MemTempDir> {
        let mut path = std::env::temp_dir();
        path.push(format!("peckish-workdir-{}", rand::random::<u64>()));
        {
            let mut fs = fs.lock().await;
            fs.create_dir_all(&path).await?;
        }

        Ok(Self { path, fs })
    }
}

impl FloppyTempDir for MemTempDir<'_> {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MemTempDir<'_> {
    fn drop(&mut self) {
        if self.path.exists() {
            run_here(async {
                let mut fs = self.fs.lock().await;
                fs.remove_dir_all(&self.path).await.unwrap();
            });
        }
    }
}

impl AsRef<Path> for MemTempDir<'_> {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<PathBuf> for MemTempDir<'_> {
    fn as_ref(&self) -> &PathBuf {
        &self.path
    }
}

impl std::ops::Deref for MemTempDir<'_> {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::io::Result;

    #[tokio::test]
    async fn test_mem_floppy_disk() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        assert_eq!("asdf", fs.read_to_string("/test.txt").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_canonicalize() -> Result<()> {
        let fs = MemFloppyDisk::new();

        assert_eq!(PathBuf::from("/"), fs.canonicalize("/").await?);
        assert_eq!(PathBuf::from("/"), fs.canonicalize("/.").await?);
        assert_eq!(PathBuf::from("/"), fs.canonicalize("/..").await?);
        assert_eq!(PathBuf::from("/"), fs.canonicalize("/../..").await?);
        assert_eq!(PathBuf::from("a"), fs.canonicalize("a").await?);
        assert_eq!(PathBuf::from("a"), fs.canonicalize("a/.").await?);
        assert_eq!(PathBuf::from("/a"), fs.canonicalize("/a/.").await?);
        assert_eq!(PathBuf::from("/a"), fs.canonicalize("/a/../a").await?);
        assert_eq!(
            PathBuf::from("/"),
            fs.canonicalize("/usr/bin/../../../../../../..").await?
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_copy() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.copy("/test.txt", "/test2.txt").await?;
        assert_eq!("asdf", fs.read_to_string("/test2.txt").await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_dir() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.create_dir("/test").await?;
        let metadata = fs.metadata("/test").await?;
        assert!(metadata.is_dir().await);

        Ok(())
    }

    #[tokio::test]
    async fn test_create_dir_all() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.create_dir_all("/test/a/b/c").await?;
        let metadata = fs.metadata("/test/a/b/c").await?;
        assert!(metadata.is_dir().await);

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
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let metadata = fs.metadata("/test.txt").await?;
        assert!(metadata.is_file().await);
        assert_eq!(4, metadata.len().await);

        Ok(())
    }

    #[tokio::test]
    async fn test_read() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let buf = fs.read("/test.txt").await?;
        assert_eq!(b"asdf", buf.as_slice());

        Ok(())
    }

    #[tokio::test]
    async fn test_read_dir() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
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
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let s = fs.read_link("/test2.txt").await?;
        assert_eq!(PathBuf::from("/test.txt"), s);

        Ok(())
    }

    #[tokio::test]
    async fn test_read_to_string() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        let s = fs.read_to_string("/test.txt").await?;
        assert_eq!("asdf", s);

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_dir() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.create_dir("/test").await?;
        fs.remove_dir("/test").await?;
        assert!(fs.metadata("/test").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_remove_dir_all() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
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
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.remove_file("/test.txt").await?;
        assert!(fs.metadata("/test.txt").await.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_rename() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
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
        assert_eq!(0o777, metadata.permissions().await.mode());

        Ok(())
    }

    #[tokio::test]
    async fn test_symlink_metadata() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let metadata = fs.symlink_metadata("/test2.txt").await?;
        assert!(metadata.is_file().await);
        assert_eq!(4, metadata.len().await);

        Ok(())
    }

    #[tokio::test]
    async fn test_symlink() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        fs.symlink("/test.txt", "/test2.txt").await?;
        let s = fs.read_link("/test2.txt").await?;
        assert_eq!(PathBuf::from("/test.txt"), s);

        Ok(())
    }

    #[tokio::test]
    async fn test_write() -> Result<()> {
        let mut fs = MemFloppyDisk::new();
        fs.write("/test.txt", "asdf").await?;
        assert_eq!("asdf", fs.read_to_string("/test.txt").await?);

        Ok(())
    }
}
