use alloc::sync::{Arc, Weak};
use core::num::NonZeroUsize;

use config::{
    board::BLOCK_SIZE,
    mm::{
        BUFFER_NEED_CACHE_CNT, MAX_BUFFER_HEADS, MAX_BUFFER_PAGES, MAX_BUFFERS_PER_PAGE, PAGE_SIZE,
        block_page_id, is_aligned_to_block,
    },
};
use device_core::BlockDevice;
use intrusive_collections::{LinkedListAtomicLink, intrusive_adapter};
use lru::LruCache;
use macro_utils::with_methods;
use sync::mutex::SpinNoIrqLock;

use crate::Page;

pub struct BufferCache {
    device: Option<Weak<dyn BlockDevice>>,
    /// Block page id to `Page`.
    // NOTE: These `Page`s are pages without file, only exist for caching pure
    // block data.
    pub pages: LruCache<usize, Arc<Page>>,
    /// Block idx to `BufferHead`.
    // NOTE: Stores all accesses to block device. Some of them will be attached
    // to pages above, while others with file related will be attached to pages
    // stored in address space.
    // PERF: perf issue will happen when lru cache is full
    // FIXME: dropped buffer head because of lru may cause trouble if it is attathed to a page, in
    // this situation, duplicate buffer head will be created
    pub buffer_heads: LruCache<usize, Arc<BufferHead>>,
}

impl BufferCache {
    pub fn new() -> Self {
        Self {
            device: None,
            pages: LruCache::new(NonZeroUsize::new(MAX_BUFFER_PAGES).unwrap()),
            buffer_heads: LruCache::new(NonZeroUsize::new(MAX_BUFFER_HEADS).unwrap()),
        }
    }

    pub fn init_device(&mut self, device: Arc<dyn BlockDevice>) {
        self.device = Some(Arc::downgrade(&device))
    }

    pub fn device(&self) -> Arc<dyn BlockDevice> {
        self.device.as_ref().unwrap().upgrade().unwrap()
    }

    pub fn read_block(&mut self, block_id: usize, buf: &mut [u8]) {
        let buffer_head = self.get_buffer_head_from_disk(block_id);
        if buffer_head.has_cached() {
            buffer_head.read_block(buf)
        } else {
            self.device().base_read_blocks(block_id, buf)
        }
    }

    pub fn write_block(&mut self, block_id: usize, buf: &[u8]) {
        let buffer_head = self.get_buffer_head_from_disk(block_id);
        if buffer_head.has_cached() {
            buffer_head.write_block(buf)
        } else {
            self.device().base_write_blocks(block_id, buf)
        }
    }

    pub fn get_buffer_head_from_disk(&mut self, block_id: usize) -> Arc<BufferHead> {
        let device = self.device();
        if let Some(buffer_head) = self.buffer_heads.get_mut(&block_id).cloned() {
            buffer_head.inc_acc_cnt();
            if buffer_head.need_cache() {
                let page = if let Some(page) = self.pages.get_mut(&block_page_id(block_id)).cloned()
                {
                    assert_eq!(page.buffer_head_cnts(), 8);
                    self.pages.pop(&block_page_id(block_id));
                    // old page will be dropped here
                    // NOTE: why we need to drop old page here, because buffer head in the old page
                    // is already dropped from `self.buffer_heads` since lru policy
                    let page = Page::new_block(&device);
                    self.pages.push(block_page_id(block_id), page.clone());
                    page
                } else {
                    let page = Page::new_block(&device);
                    self.pages.push(block_page_id(block_id), page.clone());
                    page
                };

                let block_id_start = block_id / MAX_BUFFERS_PER_PAGE * MAX_BUFFERS_PER_PAGE;
                device.base_read_blocks(block_id_start, page.bytes_array());
                for block_id_it in block_id_start..block_id_start + MAX_BUFFERS_PER_PAGE {
                    let buffer_head_it = self.get_buffer_head_or_create(block_id_it);
                    page.insert_buffer_head(buffer_head_it.clone());
                }
            }
            buffer_head.clone()
        } else {
            let buffer_head = BufferHead::new_arc(block_id);
            buffer_head.inc_acc_cnt();
            self.buffer_heads.push(block_id, buffer_head.clone());
            buffer_head
        }
    }

    pub fn get_buffer_head_or_create(&mut self, block_id: usize) -> Arc<BufferHead> {
        if let Some(buffer_head) = self.buffer_heads.get_mut(&block_id) {
            buffer_head.clone()
        } else {
            let buffer_head = BufferHead::new_arc(block_id);
            self.buffer_heads.push(block_id, buffer_head.clone());
            buffer_head
        }
    }
}

pub struct BufferHead {
    /// Block index on the device.
    block_id: usize,
    page_link: LinkedListAtomicLink,
    inner: SpinNoIrqLock<BufferHeadInner>,
}

intrusive_adapter!(pub BufferHeadAdapter = Arc<BufferHead>: BufferHead { page_link: LinkedListAtomicLink });

pub struct BufferHeadInner {
    /// Count of access before cached.
    acc_cnt: usize,
    /// Buffer state.
    bstate: BufferState,
    /// Page cache which holds the actual buffer data.
    page: Weak<Page>,
    /// Offset in page, aligned with `BLOCK_SIZE`.
    offset: usize,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    #[default]
    UnInit,
    Sync,
    Dirty,
}

impl BufferHead {
    pub fn new(block_id: usize) -> Self {
        Self {
            block_id,
            page_link: LinkedListAtomicLink::new(),
            inner: SpinNoIrqLock::new(BufferHeadInner {
                acc_cnt: 0,
                bstate: BufferState::UnInit,
                page: Weak::new(),
                offset: 0,
            }),
        }
    }

    pub fn new_arc(block_id: usize) -> Arc<Self> {
        Arc::new(Self::new(block_id))
    }

    pub fn init(&self, page: &Arc<Page>, offset: usize) {
        if self.has_cached() {
            log::error!(
                "block id {} already cached, with acc_cnt {}, page kind {:?}",
                self.block_id,
                self.acc_cnt(),
                self.page().kind()
            );
        }
        debug_assert!(is_aligned_to_block(offset) && offset < PAGE_SIZE);

        self.with_mut_inner(|inner| {
            inner.bstate = BufferState::Sync;
            inner.page = Arc::downgrade(page);
            inner.offset = offset;
        });
    }

    pub fn reset(&self) {
        self.with_mut_inner(|inner| {
            inner.acc_cnt = 0;
            inner.bstate = BufferState::UnInit;
            inner.page = Weak::new();
            inner.offset = 0;
        });
    }

    pub fn block_id(&self) -> usize {
        self.block_id
    }

    pub fn acc_cnt(&self) -> usize {
        self.inner.lock().acc_cnt
    }

    pub fn inc_acc_cnt(&self) {
        self.inner.lock().acc_cnt += 1
    }

    pub fn need_cache(&self) -> bool {
        self.acc_cnt() >= BUFFER_NEED_CACHE_CNT && !self.has_cached()
    }

    pub fn bstate(&self) -> BufferState {
        self.inner.lock().bstate
    }

    pub fn set_bstate(&self, bstate: BufferState) {
        self.inner.lock().bstate = bstate
    }

    pub fn page(&self) -> Arc<Page> {
        self.inner.lock().page.upgrade().unwrap()
    }

    pub fn offset(&self) -> usize {
        debug_assert!(self.has_cached());
        self.inner.lock().offset
    }

    pub fn has_cached(&self) -> bool {
        self.inner.lock().bstate != BufferState::UnInit
    }

    pub fn bytes_array(&self) -> &'static mut [u8] {
        let offset = self.offset();
        self.page().bytes_array_range(offset..offset + BLOCK_SIZE)
    }

    pub fn read_block(&self, buf: &mut [u8]) {
        buf.copy_from_slice(self.bytes_array())
    }

    pub fn write_block(&self, buf: &[u8]) {
        self.bytes_array().copy_from_slice(buf);
        self.set_bstate(BufferState::Dirty)
    }

    with_methods!(inner: BufferHeadInner);
}
