// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The core module for kernel memory management.
//!
//! Defines some core data-types and implements
//! a bunch of different allocators for use in the system.
//!
//! From a high level the four most interesting types in here are:
//!  * The Frame: Which is represents a block of physical memory, it's always
//!    aligned to, and a multiple of a base-page. Ideally Rust affine types
//!    should ensure we always only have one Frame covering a block of memory.
//!  * The NCache: A big stack of base and large-pages.
//!  * The TCache: A smaller stack of base and large-pages.
//!  * The KernelAllocator: Which implements GlobalAlloc.
use core::alloc::{GlobalAlloc, Layout};
use core::intrinsics::likely;
use core::ptr;
use core::sync::atomic::AtomicU64;

use log::{debug, error, trace, warn};
use slabmalloc::{Allocator, ZoneAllocator};

use crate::kcb;
use crate::prelude::*;

use backends::PhysicalPageProvider;
pub(crate) use frame::Frame;
pub(crate) use kpi::MemType;
use vspace::MapAction;

/// Re-export arch specific memory definitions
pub(crate) use crate::arch::memory::{
    kernel_vaddr_to_paddr, paddr_to_kernel_vaddr, PAddr, VAddr, BASE_PAGE_SIZE, KERNEL_BASE,
    LARGE_PAGE_SIZE,
};

pub mod backends;
pub mod detmem;
pub mod emem;
pub mod frame;
pub mod global;
pub mod mcache;
pub mod utils;
pub mod vspace;
#[cfg(test)]
pub mod vspace_model;

/// The global allocator in the kernel.
//#[cfg(not(any(test, fuzzing)))]
#[cfg(target_os = "none")]
#[global_allocator]
static MEM_PROVIDER: KernelAllocator = KernelAllocator {
    big_objects_sbrk: AtomicU64::new(
        KERNEL_BASE + (2048 * x86::bits64::paging::HUGE_PAGE_SIZE) as u64,
    ),
};

/// Different types of allocator that the KernelAllocator can use.
#[derive(Debug, PartialEq)]
enum AllocatorType {
    /// An instance of slabmalloc::ZoneAllocator
    Zone,
    /// A memory manager that implements trait XX.
    MemManager,
    /// Large regions that get map in the kernel VSpace by the `KernelAllocator`.
    MapBig,
}

/// Implements the kernel memory allocation strategy.
pub(crate) struct KernelAllocator {
    big_objects_sbrk: AtomicU64,
}

impl KernelAllocator {
    /// Try to allocate a piece of memory.
    fn try_alloc(&self, layout: Layout) -> Result<ptr::NonNull<u8>, KError> {
        let kcb = kcb::try_get_kcb().ok_or(KError::KcbUnavailable)?;
        match KernelAllocator::allocator_for(layout) {
            AllocatorType::Zone if layout.size() <= ZoneAllocator::MAX_ALLOC_SIZE => {
                // TODO(rust): Silly code duplication follows if/else
                if core::intrinsics::unlikely(kcb.in_panic_mode) {
                    let mut zone_allocator = kcb.ezone_allocator()?;
                    zone_allocator.allocate(layout).map_err(|e| e.into())
                } else {
                    let mut zone_allocator = kcb.zone_allocator()?;
                    zone_allocator.allocate(layout).map_err(|e| e.into())
                }
            }
            AllocatorType::MemManager if layout.size() <= LARGE_PAGE_SIZE => {
                let mut pmanager = kcb.try_mem_manager()?;
                let f = pmanager.allocate_large_page()?;
                unsafe { Ok(ptr::NonNull::new_unchecked(f.kernel_vaddr().as_mut_ptr())) }
            }
            AllocatorType::MapBig => {
                // Big objects are mapped into the kernel address space

                // This needs some <3:
                // * TODO(safety): Assumptions are PML4 slot 129 (big_objects_sbrk) is always free for MapBig
                // * TODO(ugly): 129 is also hard-coded in process creation
                // * TODO(safety): No bounds checking
                // * TODO(smp): Needs a spin-lock for multi-core
                // * TODO(checks): we want this case to be rare so if we end up with more than ~20
                //   big objects we should print ag warning (and start rethinking this)
                // * TODO(limitation): We can't really allocate more than what fits in a TCache

                // Figure out how much we need to map:
                let (mut base, mut large) = KernelAllocator::layout_to_pages(layout);

                // TODO(hack): Fetching more than 254 base pages would exhaust our TCache so might
                // as well get a large-page instead:
                // Slightly better: Should at least have well defined constants for `254`
                // A bit better: TCache should probably have more space base pages (like 2MiB of base pages?)
                // More better: If we need more pages than what fits in the TCache, we should get it directly
                // from the NCache?
                // Even Better: Find a good way to express this API, and maybe the whole GlobalAllocator
                // infrastructure that doesn't require estimating the pages upfront?
                if base > 254 {
                    base = 0;
                    large += 1;
                }
                // TODO(correctness): Make sure we have 20 pages for page-tables
                // so vspace ops don't fail us :/
                self.maybe_refill_tcache(base + 20, large)?;

                // We allocate (large+1) * large-page-size
                // the +1 is to account for space for all the base-pages
                // and to make sure next time we're still aligned to a 2 MiB
                // boundary
                let mut start_at = self.big_objects_sbrk.fetch_add(
                    ((large + 1) * LARGE_PAGE_SIZE) as u64,
                    core::sync::atomic::Ordering::SeqCst,
                );
                trace!(
                    "Got a large allocation {:?}, need bp {} lp {} {:#x}",
                    layout,
                    base,
                    large,
                    start_at
                );

                let base_ptr = unsafe { ptr::NonNull::new_unchecked(start_at as *mut u8) };

                let mut kvspace = crate::arch::vspace::INITIAL_VSPACE.lock();
                for _ in 0..large {
                    let mut pmanager = kcb.try_mem_manager()?;
                    let f = pmanager
                        .allocate_large_page()
                        .expect("Can't run out of memory");
                    drop(pmanager); // `map_generic` might try to re-acquire mem_manager

                    kvspace
                        .map_generic(
                            VAddr::from(start_at),
                            (f.base, f.size()),
                            MapAction::ReadWriteKernel,
                            true,
                        )
                        .expect("Can't create the mapping");

                    start_at += LARGE_PAGE_SIZE as u64;
                }

                for _ in 0..base {
                    let mut pmanager = kcb.try_mem_manager()?;
                    let f = pmanager
                        .allocate_base_page()
                        .expect("Can't run out of memory");
                    drop(pmanager); // `map_generic` might try to re-acquire mem_manager

                    kvspace
                        .map_generic(
                            VAddr::from(start_at),
                            (f.base, f.size()),
                            MapAction::ReadWriteKernel,
                            true,
                        )
                        .expect("Can't create the mapping");
                    start_at += BASE_PAGE_SIZE as u64;
                }

                Ok(base_ptr)
            }
            _ => unimplemented!("Unable to handle this allocation request {:?}", layout),
        }
    }

    /// Determines which Allocator to use for a given Layout.
    fn allocator_for(layout: Layout) -> AllocatorType {
        const MAX_ALLOC_PLUS_ONE: usize = ZoneAllocator::MAX_ALLOC_SIZE + 1;
        match layout.size() {
            0..=ZoneAllocator::MAX_ALLOC_SIZE => AllocatorType::Zone,
            MAX_ALLOC_PLUS_ONE..=LARGE_PAGE_SIZE => AllocatorType::MemManager,
            _ => AllocatorType::MapBig,
        }
    }

    /// Try to refill our core-local zone allocator.
    ///
    /// We come here if a previous allocation failed.
    fn try_refill(&self, layout: Layout, e: KError) -> Result<(), KError> {
        match (KernelAllocator::allocator_for(layout), e) {
            (AllocatorType::Zone, KError::CacheExhausted) => {
                let (needed_base_pages, needed_large_pages) =
                    KernelAllocator::refill_amount(layout);
                self.maybe_refill_tcache(needed_base_pages, needed_large_pages)?;
                self.try_refill_zone(layout)
            }
            (AllocatorType::MapBig, _) => {
                let (needed_base_pages, needed_large_pages) =
                    KernelAllocator::refill_amount(layout);
                KernelAllocator::try_refill_tcache(
                    needed_base_pages,
                    needed_large_pages,
                    MemType::Mem,
                )
            }
            (AllocatorType::MemManager, _) => {
                let (needed_base_pages, needed_large_pages) =
                    KernelAllocator::refill_amount(layout);
                KernelAllocator::try_refill_tcache(
                    needed_base_pages,
                    needed_large_pages,
                    MemType::Mem,
                )
            }
            (AllocatorType::Zone, _) => unreachable!("Not sure how to handle"),
        }
    }

    /// Calculate how many base and large pages we need to fit a Layout.
    fn layout_to_pages(layout: Layout) -> (usize, usize) {
        utils::size_to_pages(layout.size())
    }

    /// Determine for a Layout how many pages we need taking into
    /// account the type of allocator that will end up handling the request.
    fn refill_amount(layout: Layout) -> (usize, usize) {
        match KernelAllocator::allocator_for(layout) {
            AllocatorType::Zone => {
                if layout.size() <= slabmalloc::ZoneAllocator::MAX_BASE_ALLOC_SIZE {
                    (1, 0)
                } else {
                    (0, 1)
                }
            }
            AllocatorType::MemManager => {
                if layout.size() <= BASE_PAGE_SIZE {
                    (1, 0)
                } else {
                    (0, 1)
                }
            }
            AllocatorType::MapBig => KernelAllocator::layout_to_pages(layout),
        }
    }

    /// Try to refill our core-local tcache.
    pub(crate) fn try_refill_tcache(
        needed_base_pages: usize,
        needed_large_pages: usize,
        mem_type: MemType,
    ) -> Result<(), KError> {
        let kcb = kcb::try_get_kcb().ok_or(KError::KcbUnavailable)?;
        if mem_type == MemType::Mem && kcb.physical_memory.gmanager.is_none() {
            // No gmanager, can't refill then, let's hope it works anyways...
            return Ok(());
        }
        if mem_type == MemType::PMem && kcb.pmem_memory.gmanager.is_none() {
            // No gmanager, can't refill then, let's hope it works anyways...
            return Ok(());
        }

        let (gmanager, mut mem_manager, affinity) = match mem_type {
            MemType::Mem => (
                kcb.physical_memory.gmanager.unwrap(),
                kcb.try_mem_manager()?,
                kcb.physical_memory.affinity as usize,
            ),
            MemType::PMem => (
                kcb.pmem_memory.gmanager.unwrap(),
                kcb.pmem_manager(),
                kcb.pmem_memory.affinity as usize,
            ),
            _ => unreachable!(),
        };
        let mut ncache = gmanager.node_caches[affinity].lock();
        // Make sure we don't overflow the TCache
        let needed_base_pages =
            core::cmp::min(mem_manager.spare_base_page_capacity(), needed_base_pages);
        let needed_large_pages =
            core::cmp::min(mem_manager.spare_large_page_capacity(), needed_large_pages);

        for _i in 0..needed_base_pages {
            let frame = ncache.allocate_base_page()?;
            mem_manager
                .grow_base_pages(&[frame])
                .expect("We ensure to not overfill the TCache above.");
        }

        for _i in 0..needed_large_pages {
            let frame = ncache.allocate_large_page()?;
            mem_manager
                .grow_large_pages(&[frame])
                .expect("We ensure to not overfill the TCache above.");
        }

        Ok(())
    }

    /// Refill TCache only if the layout will exhaust the cache's current
    /// stored memory
    ///
    /// `let (needed_base_pages, needed_large_pages) = KernelAllocator::refill_amount(layout);`
    fn maybe_refill_tcache(
        &self,
        needed_base_pages: usize,
        needed_large_pages: usize,
    ) -> Result<(), KError> {
        let kcb = kcb::try_get_kcb().ok_or(KError::KcbUnavailable)?;
        let mem_manager = kcb.try_mem_manager()?;

        let free_bp = mem_manager.free_base_pages();
        let free_lp = mem_manager.free_large_pages();

        // Dropping things, as they'll get reacquired in try_refill_tcache
        drop(mem_manager);

        if needed_base_pages > free_bp || needed_large_pages > free_lp {
            debug!(
                "Refilling the TCache: needed_bp {} needed_lp {} free_bp {} free_lp {}",
                needed_base_pages, needed_large_pages, free_bp, free_lp
            );
            KernelAllocator::try_refill_tcache(needed_base_pages, needed_large_pages, MemType::Mem)
        } else {
            debug!(
                "Refilling unnecessary: needed_bp {} needed_lp {} free_bp {} free_lp {}",
                needed_base_pages, needed_large_pages, free_bp, free_lp
            );

            Ok(())
        }
    }

    /// Try refill zone
    fn try_refill_zone(&self, layout: Layout) -> Result<(), KError> {
        let kcb = kcb::try_get_kcb().ok_or(KError::KcbUnavailable)?;
        let needs_a_base_page = layout.size() <= slabmalloc::ZoneAllocator::MAX_BASE_ALLOC_SIZE;

        let mut mem_manager = kcb.try_mem_manager()?;
        // TODO(rust): Silly code duplication follows if/else
        if core::intrinsics::unlikely(kcb.in_panic_mode) {
            let mut zone = kcb.ezone_allocator()?;
            if needs_a_base_page {
                let frame = mem_manager.allocate_base_page()?;
                unsafe {
                    let base_page_ptr: *mut slabmalloc::ObjectPage =
                        frame.uninitialized::<slabmalloc::ObjectPage>().as_mut_ptr();
                    zone.refill(layout, &mut *base_page_ptr)
                        .expect("This should always succeed");
                }
            } else {
                // Needs a large page
                let frame = mem_manager.allocate_large_page()?;
                unsafe {
                    let large_page_ptr: *mut slabmalloc::LargeObjectPage = frame
                        .uninitialized::<slabmalloc::LargeObjectPage>()
                        .as_mut_ptr();
                    zone.refill_large(layout, &mut *large_page_ptr)
                        .expect("This should always succeed");
                }
            }
        } else {
            let mut zone = kcb.zone_allocator()?;
            if needs_a_base_page {
                let frame = mem_manager.allocate_base_page()?;
                unsafe {
                    let base_page_ptr: *mut slabmalloc::ObjectPage =
                        frame.uninitialized::<slabmalloc::ObjectPage>().as_mut_ptr();
                    zone.refill(layout, &mut *base_page_ptr)
                        .expect("This should always succeed");
                }
            } else {
                // Needs a large page
                let frame = mem_manager.allocate_large_page()?;
                unsafe {
                    let large_page_ptr: *mut slabmalloc::LargeObjectPage = frame
                        .uninitialized::<slabmalloc::LargeObjectPage>()
                        .as_mut_ptr();
                    zone.refill_large(layout, &mut *large_page_ptr)
                        .expect("This should always succeed");
                }
            }
        }
        Ok(())
    }
}

/// Implementation of GlobalAlloc for the kernel.
///
/// The algorithm in alloc/dealloc should take care of allocating kernel objects of
/// various sizes and is responsible for balancing the memory between different
/// allocators.
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        for _tries in 0..3 {
            let res = self.try_alloc(layout);
            match res {
                // Allocation worked
                Ok(nptr) => {
                    return nptr.as_ptr();
                }
                Err(KError::KcbUnavailable) => {
                    unreachable!(
                        "Bug; trying to get KCB 2x in during `try_alloc` {:?}",
                        layout
                    );
                }
                Err(KError::ManagerAlreadyBorrowed) => {
                    unreachable!(
                        "ManagerAlreadyBorrowed trying to get mem manager 2x during `try_alloc`"
                    );
                }
                Err(e) => {
                    // Allocation didn't work, we try to refill
                    match self.try_refill(layout, e) {
                        Ok(_) => {
                            // Refilling worked, re-try allocation
                            continue;
                        }
                        Err(KError::KcbUnavailable) => {
                            error!("KcbUnavailable trying to get KCB during `try_refill`");
                            break;
                        }
                        Err(KError::ManagerAlreadyBorrowed) => {
                            error!("ManagerAlreadyBorrowed trying to get mem manager 2x during `try_refill` {:?}", layout);
                            break;
                        }
                        Err(_e) => {
                            // Refilling failed, re-try allocation
                            return ptr::null_mut();
                        }
                    }
                }
            }
        }

        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        crate::kcb::try_get_kcb().map_or_else(
            || {
                unreachable!("Trying to deallocate {:p} {:?} without a KCB.", ptr, layout);
            },
            |kcb| {
                if layout.size() <= ZoneAllocator::MAX_ALLOC_SIZE {
                    // TODO(rust): Silly code duplication follows if/else
                    if core::intrinsics::unlikely(kcb.in_panic_mode) {
                        let mut zone_allocator = kcb
                            .ezone_allocator()
                            .expect("Can't borrow ezone_allocator?");
                        if likely(!ptr.is_null()) {
                            zone_allocator
                                .deallocate(ptr::NonNull::new_unchecked(ptr), layout)
                                .expect("Can't deallocate?");
                        } else {
                            warn!("Ignore null pointer deallocation");
                        }
                    } else {
                        let mut zone_allocator =
                            kcb.zone_allocator().expect("Can't borrow zone_allocator?");
                        if likely(!ptr.is_null()) {
                            zone_allocator
                                .deallocate(ptr::NonNull::new_unchecked(ptr), layout)
                                .expect("Can't deallocate?");
                        } else {
                            warn!("Ignore null pointer deallocation");
                        }
                    }
                } else {
                    let kcb = kcb::get_kcb();
                    let mut fmanager = kcb.mem_manager();

                    if layout.size() <= BASE_PAGE_SIZE {
                        assert!(layout.align() <= BASE_PAGE_SIZE);
                        let frame = Frame::new(
                            kernel_vaddr_to_paddr(VAddr::from_u64(ptr as u64)),
                            BASE_PAGE_SIZE,
                            // TODO(numa-correctness): This is not necessarily correct as free can happen
                            // while `physical_memory` changes to different affinities
                            // we try to avoid this at the moment by being careful about freeing things
                            // during changes to allocation affinity (the NCache or TCache would panic)
                            kcb.physical_memory.affinity,
                        );

                        match fmanager.release_base_page(frame) {
                            Ok(_) => { /* Frame addition to tcache as successful.*/ }
                            Err(_e) => match kcb.physical_memory.gmanager {
                                // Try adding frame to ncache.
                                Some(gmanager) => {
                                    let mut ncache =
                                        gmanager.node_caches[frame.affinity as usize].lock();
                                    ncache
                                        .release_base_page(frame)
                                        .expect("Can't deallocate frame");
                                }
                                None => unreachable!("Unable to access global memory manager"),
                            },
                        }
                    } else if layout.size() <= LARGE_PAGE_SIZE {
                        assert!(layout.align() <= LARGE_PAGE_SIZE);
                        let frame = Frame::new(
                            kernel_vaddr_to_paddr(VAddr::from_u64(ptr as u64)),
                            LARGE_PAGE_SIZE,
                            // TODO(numa-correctness): This is not necessarily correct as free can happen
                            // while `physical_memory` changes to different affinities
                            // we try to avoid this at the moment by being careful about freeing things
                            // during changes to allocation affinity (the NCache or TCache would panic)
                            kcb.physical_memory.affinity,
                        );

                        fmanager
                            .release_large_page(frame)
                            .expect("Can't deallocate frame");
                    } else {
                        error!("Loosing large memory region. Oh well.")
                    }
                }
            },
        );
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        crate::kcb::try_get_kcb().map_or_else(
            || {
                unreachable!("Trying to reallocate {:p} {:?} without a KCB.", ptr, layout);
            },
            |kcb| {
                if !kcb.in_panic_mode
                    && layout.size() <= ZoneAllocator::MAX_ALLOC_SIZE
                    && layout.size() != BASE_PAGE_SIZE
                    && new_size <= ZoneAllocator::get_max_size(layout.size()).unwrap_or(0x0)
                {
                    // Don't do a re-allocation if we're in a big enough size-class
                    // in the ZoneAllocator
                    ptr
                } else {
                    // Slow path, allocate a bigger region and de-allocate the old one
                    let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
                    let new_ptr = self.alloc(new_layout);
                    if !new_ptr.is_null() {
                        ptr::copy_nonoverlapping(
                            ptr,
                            new_ptr,
                            core::cmp::min(layout.size(), new_size),
                        );
                        self.dealloc(ptr, layout);
                    }
                    new_ptr
                }
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_to_pages() {
        let l = unsafe { Layout::from_size_align_unchecked(BASE_PAGE_SIZE - 1, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (1, 0));

        let l = unsafe { Layout::from_size_align_unchecked(BASE_PAGE_SIZE, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (1, 0));

        let l = unsafe { Layout::from_size_align_unchecked(BASE_PAGE_SIZE + 1, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (2, 0));

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE - 1, 0) };
        assert_eq!(
            KernelAllocator::layout_to_pages(l),
            (LARGE_PAGE_SIZE / BASE_PAGE_SIZE, 0)
        );

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (0, 1));

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE + 1, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (1, 1));

        let l =
            unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE + 10 * BASE_PAGE_SIZE, 0) };
        assert_eq!(KernelAllocator::layout_to_pages(l), (10, 1));

        let l = unsafe {
            Layout::from_size_align_unchecked(2 * LARGE_PAGE_SIZE + 50 * BASE_PAGE_SIZE, 0)
        };
        assert_eq!(KernelAllocator::layout_to_pages(l), (50, 2));
    }

    #[test]
    fn allocator_selection() {
        let l = unsafe { Layout::from_size_align_unchecked(8, 8) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::Zone);

        let l = unsafe { Layout::from_size_align_unchecked(BASE_PAGE_SIZE, BASE_PAGE_SIZE) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::Zone);

        let l = unsafe { Layout::from_size_align_unchecked(BASE_PAGE_SIZE + 1, BASE_PAGE_SIZE) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::Zone);

        let l = unsafe { Layout::from_size_align_unchecked(153424, 8) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::MemManager);

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE - 1, LARGE_PAGE_SIZE) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::MemManager);

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE, LARGE_PAGE_SIZE) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::MemManager);

        let l = unsafe { Layout::from_size_align_unchecked(LARGE_PAGE_SIZE + 1, LARGE_PAGE_SIZE) };
        assert_eq!(KernelAllocator::allocator_for(l), AllocatorType::MapBig);
    }
}
