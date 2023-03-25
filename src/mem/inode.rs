use std::path::PathBuf;
use std::time::SystemTime;

use derive_getters::Getters;

use super::MemPermissions;

#[derive(Getters, Debug, PartialEq, Eq)]
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

    #[allow(unused)]
    pub fn buffer_view(&self) -> &[u8] {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InodeType {
    File,
    Dir,
    Symlink,
}
