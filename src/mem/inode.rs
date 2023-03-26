use std::path::PathBuf;
use std::time::SystemTime;

use derive_getters::Getters;

use super::MemPermissions;

#[derive(Getters, Debug, PartialEq, Eq, Clone)]
pub(crate) struct Inode {
    serial: u64,
    kind: InodeType,
    mode: MemPermissions,
    path: PathBuf,
    uid: u32,
    gid: u32,
    size: u64,
    atime: SystemTime,
    mtime: SystemTime,
    ctime: SystemTime,
    buffer: Vec<u8>,
    symlink_target: Option<PathBuf>,
}

impl Ord for Inode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl PartialOrd for Inode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Inode {
    pub fn set_serial(&mut self, serial: u64) {
        self.serial = serial;
    }

    pub fn len(&self) -> u64 {
        self.buffer.len() as u64
    }

    pub fn set_len(&mut self, size: u64) {
        self.size = size;
    }

    pub fn set_permissions(&mut self, mode: MemPermissions) {
        self.mode = mode;
    }

    pub fn permissions(&self) -> MemPermissions {
        self.mode.clone()
    }

    pub fn buffer_view(&self) -> &[u8] {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    pub fn new_dir<P: Into<PathBuf>>(next_inode: u64, path: P) -> Self {
        Self {
            serial: next_inode,
            kind: InodeType::Dir,
            mode: MemPermissions { mode: 0o755 },
            path: path.into(),
            uid: 0,
            gid: 0,
            size: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            buffer: Vec::new(),
            symlink_target: None,
        }
    }

    pub fn new_file<P: Into<PathBuf>, V: Into<Vec<u8>>>(next_inode: u64, path: P, data: V) -> Self {
        Self {
            serial: next_inode,
            kind: InodeType::File,
            mode: MemPermissions { mode: 0o644 },
            path: path.into(),
            uid: 0,
            gid: 0,
            size: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            buffer: data.into(),
            symlink_target: None,
        }
    }

    pub fn new_symlink<P: Into<PathBuf>>(next_inode: u64, path: P, target: P) -> Self {
        Self {
            serial: next_inode,
            kind: InodeType::Symlink,
            mode: MemPermissions { mode: 0o777 },
            path: path.into(),
            uid: 0,
            gid: 0,
            size: 0,
            atime: SystemTime::now(),
            mtime: SystemTime::now(),
            ctime: SystemTime::now(),
            buffer: Vec::new(),
            symlink_target: Some(target.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub(crate) enum InodeType {
    File,
    Dir,
    Symlink,
}
