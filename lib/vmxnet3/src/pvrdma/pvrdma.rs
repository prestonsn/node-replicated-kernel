// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: BSD-2-Clause

/* originally generated by rust-bindgen 0.58.1 from pvrdma.h in Linux v5.12 */

#![allow(non_camel_case_types)]

use x86::current::paging::{PAddr, BASE_PAGE_SHIFT};

use super::pci::paddr_to_kernel_vaddr;

pub type be16 = u16;
pub type be32 = u32;
pub type be64 = u64;

/// VMware PVRDMA PCI device id.
pub const PCI_DEVICE_ID_VMWARE_PVRDMA: u32 = 2080;

/// Default number of pages allocated for async event ring & completion queue
/// event ring
pub const PVRDMA_NUM_RING_PAGES: u32 = 4;

pub const PVRDMA_QP_NUM_HEADER_PAGES: u32 = 1;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_uar_map {
    pub pfn: u64,
    pub map: *mut u64,
    pub index: usize,
}

impl pvrdma_uar_map {
    pub fn new(paddr: u64) -> Self {
        let pfn = paddr >> BASE_PAGE_SHIFT;
        let map = paddr_to_kernel_vaddr(PAddr::from(pfn << BASE_PAGE_SHIFT));
        Self {
            pfn,
            map: map.as_mut_ptr(),
            index: 0,
        }
    }
}
