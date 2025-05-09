use alloc::{boxed::Box, ffi::CString, string::String, sync::Arc, vec, vec::Vec};
use core::{
    cmp,
    sync::atomic::{AtomicUsize, Ordering},
    usize,
};

use async_trait::async_trait;
use config::{
    board::BLOCK_SIZE,
    mm::{
        MAX_BUFFERS_PER_PAGE, PAGE_MASK, PAGE_SIZE, align_offset_to_page, block_page_id,
        round_down_to_page, round_up_to_page,
    },
};
use downcast_rs::{DowncastSync, impl_downcast};
use memory::address;
use page::Page;
use spin::Mutex;
use systype::{SysError, SysResult, SyscallResult};

use crate::{
    Dentry, DirEntry, Inode, InodeState, InodeType, OpenFlags, PollEvents, SeekFrom, SuperBlock,
    inode,
};

pub struct FileMeta {
    /// Dentry which pointes to this file.
    pub dentry: Arc<dyn Dentry>,
    pub inode: Arc<dyn Inode>,

    /// Offset position of this file.
    /// WARN: may cause trouble if this is not locked with other things.
    pub pos: AtomicUsize,
    pub flags: Mutex<OpenFlags>,
}

impl FileMeta {
    pub fn new(dentry: Arc<dyn Dentry>, inode: Arc<dyn Inode>) -> Self {
        Self {
            dentry,
            inode,
            pos: 0.into(),
            flags: Mutex::new(OpenFlags::empty()),
        }
    }
}

#[async_trait]
pub trait File: Send + Sync + DowncastSync {
    fn meta(&self) -> &FileMeta;

    async fn base_read_at(&self, _offset: usize, _buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    async fn base_write_at(&self, _offset: usize, _buf: &[u8]) -> SyscallResult {
        todo!()
    }

    /// Read directory entries. This is called by the getdents(2) system call.
    ///
    /// For every call, this function will return an valid entry, or an error.
    /// If it read to the end of directory, it will return an empty entry.
    fn base_read_dir(&self) -> SysResult<Option<DirEntry>> {
        todo!()
    }

    /// Load all dentry and inodes in a directory. Will not advance dir offset.
    fn base_load_dir(&self) -> SysResult<()> {
        todo!()
    }

    /// Read a page at `offset_aligned` without page cache.
    async fn read_page_at(&self, offset_aligned: usize) -> SysResult<Option<Arc<Page>>> {
        log::trace!("[File::read_page] read offset {offset_aligned}");

        if offset_aligned >= self.size() {
            log::warn!("[File::read_page] reach end of file");
            return Ok(None);
        }

        let inode = self.inode();
        let page_cache = inode.page_cache().unwrap();

        let device = inode.super_block().device();
        let page = Page::new_file(&device);
        // read a page normally or less than a page when EOF reached
        let len = self
            .base_read_at(offset_aligned, page.bytes_array())
            .await?;

        // let virtio_blk = device
        //     .downcast_arc::<VirtIoBlkDev>()
        //     .unwrap_or_else(|_| unreachable!());
        // let mut buffer_caches = virtio_blk.cache.lock();
        // for offset in (offset_aligned..offset_aligned + len).step_by(BLOCK_SIZE) {
        //     let block_id = inode.get_blk_idx(offset)?;
        //     let buffer_head = buffer_caches.get_buffer_head_or_create(block_id as
        // usize);     if buffer_head.has_cached() {
        //         log::warn!("read page conflict");
        //         // only block cache can be transfered to file cache, e.g. a directory
        // is         // mis recognized as block cache
        //         assert!(buffer_head.page().kind().is_block_cache());
        //         buffer_caches.pages.pop(&block_page_id(block_id));
        //         // block page should be dropped here
        //     }
        //     page.insert_buffer_head(buffer_head);
        // }

        page_cache.insert_page(offset_aligned, page.clone());

        Ok(Some(page))
    }

    /// Read at an `offset`, and will fill `buf` until `buf` is full or eof is
    /// reached. Will not advance offset.
    ///
    /// Returns count of bytes actually read or an error.
    async fn read_at(&self, offset: usize, buf: &mut [u8]) -> SyscallResult {
        log::info!(
            "[File::read] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );

        let inode = self.inode();

        let Some(page_cache) = inode.page_cache() else {
            log::debug!("[File::read] read without address_space");
            let count = self.base_read_at(offset, buf).await?;
            return Ok(count);
        };

        let mut buf_it = buf;
        let mut offset_it = offset;

        log::debug!("[File::read] read with address_space");
        while !buf_it.is_empty() && offset_it < self.size() {
            let (offset_aligned, offset_in_page) = align_offset_to_page(offset_it);
            let page = if let Some(page) = page_cache.get_page(offset_aligned) {
                page
            } else if let Some(page) = self.read_page_at(offset_aligned).await? {
                page
            } else {
                // no page means EOF
                break;
            };
            let len = (buf_it.len())
                .min(PAGE_SIZE - offset_in_page)
                .min(self.size() - offset_it);
            buf_it[0..len]
                .copy_from_slice(page.bytes_array_range(offset_in_page..offset_in_page + len));
            log::trace!("[File::read] read count {len}, buf len {}", buf_it.len());
            offset_it += len;
            buf_it = &mut buf_it[len..];
        }
        log::info!("[File::read] read count {}", offset_it - offset);
        Ok(offset_it - offset)
    }

    async fn get_page_at(&self, offset_aligned: usize) -> SysResult<Option<Arc<Page>>> {
        let inode = self.inode();
        let page_cache = inode.page_cache().unwrap();
        if let Some(page) = page_cache.get_page(offset_aligned) {
            Ok(Some(page))
        } else if let Some(page) = self.read_page_at(offset_aligned).await? {
            Ok(Some(page))
        } else {
            // no page means EOF
            Ok(None)
        }
    }

    /// Called by write(2) and related system calls.
    ///
    /// On success, the number of bytes written is returned, and the file offset
    /// is incremented by the number of bytes actually written.
    async fn write_at(&self, offset: usize, buf: &[u8]) -> SyscallResult {
        log::info!(
            "[File::write] file {}, offset {offset}, buf len {}",
            self.dentry().path(),
            buf.len()
        );

        let inode = self.inode();
        inode.set_state(InodeState::Dirty);

        let Some(page_cache) = inode.page_cache() else {
            log::debug!("[File::write] write without address_space");
            let count = self.base_write_at(offset, buf).await?;
            if offset + count > inode.size() {
                inode.set_size(offset + count);
            }
            return Ok(count);
        };

        if offset > self.size() {
            todo!("offset greater than size, will create hole");
        }

        let device = self.super_block().device();
        let mut buf_it = buf;
        let mut offset_it = offset;

        while !buf_it.is_empty() {
            let (offset_aligned, offset_in_page) = align_offset_to_page(offset_it);
            let page = if let Some(page) = page_cache.get_page(offset_aligned) {
                page
            } else if let Some(page) = self.read_page_at(offset_aligned).await? {
                page
            } else {
                log::info!("[File::write_at] create new page");
                let page = Page::new_file(&device);
                page_cache.insert_page(offset_aligned, page.clone());
                page
            };
            let len = (buf_it.len()).min(PAGE_SIZE - offset_in_page);
            page.bytes_array_range(offset_in_page..offset_in_page + len)
                .copy_from_slice(&buf_it[0..len]);
            log::trace!("[File::write] write count {len}, buf len {}", buf_it.len());
            offset_it += len;
            buf_it = &buf_it[len..];
        }
        if offset_it > self.size() {
            log::warn!(
                "[File::write_at] write beyond file, offset_it:{offset_it}, size:{}",
                self.size()
            );
            self.base_write_at(self.size(), &buf[self.size() - offset..])
                .await?;
            let old_size = self.size();
            let new_size = offset_it;
            // let virtio_blk = device
            //     .downcast_arc::<VirtIoBlkDev>()
            //     .unwrap_or_else(|_| unreachable!());
            // let mut buffer_caches = virtio_blk.cache.lock();
            // for offset_aligned_page in
            // (round_down_to_page(old_size)..new_size).step_by(PAGE_SIZE) {
            //     let page = page_cache.get_page(offset_aligned_page).unwrap();
            //     for i in page.buffer_head_cnts()..MAX_BUFFERS_PER_PAGE {
            //         let offset_aligned_block = offset_aligned_page + i * BLOCK_SIZE;
            //         if offset_aligned_block < new_size {
            //             let blk_idx = inode.get_blk_idx(offset_aligned_block)?;
            //             log::debug!("offset {offset_aligned_block}, blk idx {blk_idx}");
            //             let buffer_head =
            // buffer_caches.get_buffer_head_or_create(blk_idx);             if
            // buffer_head.has_cached() {                 log::warn!("write page
            // conflict");                 // only block cache can be transfered
            // to file cache, e.g. a directory is                 // mis
            // recognized as block cache
            // assert!(buffer_head.page().kind().is_block_cache());
            // buffer_caches.pages.pop(&block_page_id(blk_idx));
            // // block page should be dropped here             }
            //             page.insert_buffer_head(buffer_head);
            //         } else {
            //             break;
            //         }
            //     }
            // }
            inode.set_size(new_size);
        }
        Ok(buf.len())
    }

    fn flush(&self) -> SysResult<usize> {
        todo!()
    }

    fn ioctl(&self, _cmd: usize, _arg: usize) -> SyscallResult {
        Err(SysError::ENOTTY)
    }

    async fn readlink(&self, buf: &mut [u8]) -> SyscallResult {
        todo!()
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    async fn base_poll(&self, events: PollEvents) -> PollEvents {
        let mut res = PollEvents::empty();
        if events.contains(PollEvents::IN) {
            res |= PollEvents::IN;
        }
        if events.contains(PollEvents::OUT) {
            res |= PollEvents::OUT;
        }
        res
    }

    fn inode(&self) -> Arc<dyn Inode> {
        self.meta().inode.clone()
    }

    fn itype(&self) -> InodeType {
        self.meta().inode.itype()
    }

    /// Called when the VFS needs to move the file position index.
    ///
    /// Return the result offset.
    ///
    /// lseek() allows the file offset to be set beyond the end of the file (but
    /// this does not change the size of the file). If data is later written at
    /// this point, subsequent reads of the data in the gap (a "hole") return
    /// null bytes ('\0') until data is actually written into the gap.
    // TODO: On Linux, using lseek() on a terminal device fails with the error
    // ESPIPE. However, many function will use this Seek.
    fn seek(&self, pos: SeekFrom) -> SyscallResult {
        let mut res_pos = self.pos();
        match pos {
            SeekFrom::Current(off) => {
                if off < 0 {
                    if res_pos as i64 - off.abs() < 0 {
                        return Err(SysError::EINVAL);
                    }
                    res_pos -= off.abs() as usize;
                } else {
                    res_pos += off as usize;
                }
            }
            SeekFrom::Start(off) => {
                res_pos = off as usize;
            }
            SeekFrom::End(off) => {
                let size = self.size();
                if off < 0 {
                    res_pos = size - off.abs() as usize;
                } else {
                    res_pos = size + off as usize;
                }
            }
        }
        self.set_pos(res_pos);
        Ok(res_pos)
    }

    fn pos(&self) -> usize {
        self.meta().pos.load(Ordering::Relaxed)
    }

    fn set_pos(&self, pos: usize) {
        self.meta().pos.store(pos, Ordering::Relaxed)
    }

    fn dentry(&self) -> Arc<dyn Dentry> {
        self.meta().dentry.clone()
    }

    fn super_block(&self) -> Arc<dyn SuperBlock> {
        self.meta().dentry.super_block()
    }

    fn size(&self) -> usize {
        self.meta().inode.size()
    }

    fn flags(&self) -> OpenFlags {
        self.meta().flags.lock().clone()
    }

    fn set_flags(&self, flags: OpenFlags) {
        *self.meta().flags.lock() = flags;
    }
}

impl dyn File {
    /// Read from offset in self, and will fill `buf` until `buf` is full or eof
    /// is reached. Will advance offset.
    pub async fn read(&self, buf: &mut [u8]) -> SyscallResult {
        let pos = self.pos();
        let ret = self.read_at(pos, buf).await?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    pub async fn write(&self, buf: &[u8]) -> SyscallResult {
        if self.flags().contains(OpenFlags::O_APPEND) {
            self.set_pos(self.size());
        }
        let pos = self.pos();
        let ret = self.write_at(pos, buf).await?;
        self.set_pos(pos + ret);
        Ok(ret)
    }

    /// Given interested events, keep track of these events and return events
    /// that is ready.
    // NOTE: async function but always returns `Ready`. Why async, to take the
    // waker.
    pub async fn poll(&self, events: PollEvents) -> PollEvents {
        log::info!("[File::poll] path:{}", self.dentry().path());
        self.base_poll(events).await
    }

    pub fn load_dir(&self) -> SysResult<()> {
        let inode = self.inode();
        if inode.state() == InodeState::UnInit {
            self.base_load_dir()?;
            inode.set_state(InodeState::Sync)
        }
        Ok(())
    }

    pub fn read_dir(&self, buf: &mut [u8]) -> SyscallResult {
        self.load_dir()?;

        #[derive(Debug, Clone, Copy)]
        #[repr(C)]
        struct LinuxDirent64 {
            d_ino: u64,
            d_off: u64,
            d_reclen: u16,
            d_type: u8,
            // d_name follows here, which will be written later
        }
        let buf_len = buf.len();
        // NOTE: Considering C struct align, we can not use `size_of` directly, because
        // `size_of::<LinuxDirent64>` equals 24, which is not what we want.
        const LEN_BEFORE_NAME: usize = 19;
        let mut writen_len = 0;
        let mut buf_it = buf;
        for dentry in self.dentry().children().values().skip(self.pos()) {
            if dentry.is_negetive() {
                self.seek(SeekFrom::Current(1))?;
                continue;
            }
            // align to 8 bytes
            let c_name_len = dentry.name().len() + 1;
            let rec_len = (LEN_BEFORE_NAME + c_name_len + 7) & !0x7;
            let inode = dentry.inode()?;
            let linux_dirent = LinuxDirent64 {
                d_ino: inode.ino() as u64,
                d_off: self.pos() as u64,
                d_type: inode.itype() as u8,
                d_reclen: rec_len as u16,
            };

            log::debug!("[sys_getdents64] linux dirent {linux_dirent:?}");
            if writen_len + rec_len > buf_len {
                break;
            }

            self.seek(SeekFrom::Current(1))?;
            let ptr = buf_it.as_mut_ptr() as *mut LinuxDirent64;
            unsafe {
                ptr.copy_from_nonoverlapping(&linux_dirent, 1);
            }
            buf_it[LEN_BEFORE_NAME..LEN_BEFORE_NAME + c_name_len - 1]
                .copy_from_slice(dentry.name().as_bytes());
            buf_it[LEN_BEFORE_NAME + c_name_len - 1] = b'\0';
            buf_it = &mut buf_it[rec_len..];
            writen_len += rec_len;
        }
        Ok(writen_len)
    }

    /// Read all data from this file synchronously.
    pub async fn read_all(&self) -> SysResult<Vec<u8>> {
        log::info!("[File::read_all] file size {}", self.size());
        let mut buf = vec![0; self.size()];
        self.read_at(0, &mut buf).await?;
        Ok(buf)
    }

    pub async fn readlink_string(&self) -> SysResult<String> {
        let mut path_buf: Vec<u8> = vec![0; 512];
        let len = self.readlink(&mut path_buf).await?;
        path_buf.truncate(len + 1);
        let path = CString::from_vec_with_nul(path_buf)
            .unwrap()
            .into_string()
            .unwrap();
        log::debug!("[File::readlink_string] read link returns {path}");
        Ok(path)
    }
}

impl_downcast!(sync File);
