use alloc::{boxed::Box, sync::Arc};

use async_trait::async_trait;
use config::board::BLOCK_SIZE;
use systype::{SysError, SysResult, SyscallResult};
use vfs_core::{
    Dentry, DentryMeta, DirEntry, File, FileMeta, Inode, InodeMeta, InodeMode, Stat, SuperBlock,
};

pub struct NullDentry {
    meta: DentryMeta,
}

impl NullDentry {
    pub fn new(
        name: &str,
        super_block: Arc<dyn SuperBlock>,
        parent: Option<Arc<dyn Dentry>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            meta: DentryMeta::new(name, super_block, parent),
        })
    }
}

impl Dentry for NullDentry {
    fn meta(&self) -> &DentryMeta {
        &self.meta
    }

    fn base_open(self: Arc<Self>) -> SysResult<Arc<dyn File>> {
        Ok(Arc::new(NullFile {
            meta: FileMeta::new(self.clone(), self.inode()?),
        }))
    }

    fn base_lookup(self: Arc<Self>, _name: &str) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_create(self: Arc<Self>, _name: &str, _mode: InodeMode) -> SysResult<Arc<dyn Dentry>> {
        Err(SysError::ENOTDIR)
    }

    fn base_unlink(self: Arc<Self>, _name: &str) -> SysResult<()> {
        Err(SysError::ENOTDIR)
    }
}

pub struct NullInode {
    meta: InodeMeta,
}

impl NullInode {
    pub fn new(super_block: Arc<dyn SuperBlock>) -> Arc<Self> {
        let size = BLOCK_SIZE;
        Arc::new(Self {
            meta: InodeMeta::new(InodeMode::CHAR, super_block, size),
        })
    }
}

impl Inode for NullInode {
    fn meta(&self) -> &InodeMeta {
        &self.meta
    }

    fn get_attr(&self) -> SysResult<Stat> {
        let inner = self.meta.inner.lock();
        let mode = self.meta.mode.bits();
        let len = inner.size;
        Ok(Stat {
            st_dev: 0,
            st_ino: self.meta.ino as u64,
            st_mode: mode,
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            __pad: 0,
            st_size: len as u64,
            st_blksize: 512,
            __pad2: 0,
            st_blocks: (len / 512) as u64,
            st_atime: inner.atime,
            st_mtime: inner.mtime,
            st_ctime: inner.ctime,
            unused: 0,
        })
    }
}

pub struct NullFile {
    meta: FileMeta,
}

#[async_trait]
impl File for NullFile {
    fn meta(&self) -> &FileMeta {
        &self.meta
    }

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        Ok(0)
    }

    async fn base_write_at(&self, _offset: usize, buf: &[u8]) -> SyscallResult {
        Ok(buf.len())
    }

    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        Err(SysError::ENOTDIR)
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }
}
