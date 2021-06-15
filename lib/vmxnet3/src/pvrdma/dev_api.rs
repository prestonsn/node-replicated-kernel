// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: BSD-2-Clause

//! API structures and constants to interact with pvrdma PCI device

/* originally generated by rust-bindgen 0.58.1 from pvrdma_dev_api.h in Linux v5.12 */

#![allow(non_camel_case_types)]

use x86::current::paging::IOAddr;

use super::pvrdma::be64;
use super::verbs::{pvrdma_port_attr, pvrdma_qp_attr, pvrdma_srq_attr};

macro_rules! bit {
    ($x:expr) => {
        1 << $x
    };
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct __BindgenBitfieldUnit<Storage> {
    storage: Storage,
}
impl<Storage> __BindgenBitfieldUnit<Storage> {
    #[inline]
    pub const fn new(storage: Storage) -> Self {
        Self { storage }
    }
}
impl<Storage> __BindgenBitfieldUnit<Storage>
where
    Storage: AsRef<[u8]> + AsMut<[u8]>,
{
    #[inline]
    pub fn get_bit(&self, index: usize) -> bool {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = self.storage.as_ref()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        byte & mask == mask
    }
    #[inline]
    pub fn set_bit(&mut self, index: usize, val: bool) {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = &mut self.storage.as_mut()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        if val {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
    #[inline]
    pub fn get(&self, bit_offset: usize, bit_width: u8) -> u64 {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());
        let mut val = 0;
        for i in 0..(bit_width as usize) {
            if self.get_bit(i + bit_offset) {
                let index = if cfg!(target_endian = "big") {
                    bit_width as usize - 1 - i
                } else {
                    i
                };
                val |= 1 << index;
            }
        }
        val
    }
    #[inline]
    pub fn set(&mut self, bit_offset: usize, bit_width: u8, val: u64) {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());
        for i in 0..(bit_width as usize) {
            let mask = 1 << i;
            let val_bit_is_set = val & mask == mask;
            let index = if cfg!(target_endian = "big") {
                bit_width as usize - 1 - i
            } else {
                i
            };
            self.set_bit(index + bit_offset, val_bit_is_set);
        }
    }
}

// PVRDMA version macros. Some new features require updates to PVRDMA_VERSION.
// These macros allow us to check for different features if necessary.
pub const PVRDMA_ROCEV1_VERSION: u32 = 17;
pub const PVRDMA_ROCEV2_VERSION: u32 = 18;
pub const PVRDMA_PPN64_VERSION: u32 = 19;
pub const PVRDMA_QPHANDLE_VERSION: u32 = 20;
pub const PVRDMA_VERSION: u32 = 20;

pub const PVRDMA_BOARD_ID: u32 = 1;
pub const PVRDMA_REV_ID: u32 = 1;

// Masks and accessors for page directory, which is a two-level lookup: page
// directory -> page table -> page. Only one directory for now, but we could
// expand that easily. 9 bits for tables, 9 bits for pages, gives one gigabyte
// for memory regions and so forth.

pub const PVRDMA_PDIR_SHIFT: u32 = 18;
pub const PVRDMA_PTABLE_SHIFT: u32 = 9;

/// #define PVRDMA_PAGE_DIR_DIR(x)		(((x) >> PVRDMA_PDIR_SHIFT) & 0x1)
pub const fn page_dir_dir(x: usize) -> usize {
    (x >> PVRDMA_PDIR_SHIFT) & 0x1
}

/// #define PVRDMA_PAGE_DIR_TABLE(x)	(((x) >> PVRDMA_PTABLE_SHIFT) & 0x1ff)
pub const fn page_dir_table(x: usize) -> usize {
    (x >> PVRDMA_PTABLE_SHIFT) & 0x1ff
}

pub const PVRDMA_PAGE_DIR_MAX_PAGES: u32 = 512 * 512;
pub const PVRDMA_MAX_FAST_REG_PAGES: u32 = 128;

/// Max MSI-X vectors.
pub const PVRDMA_MAX_INTERRUPTS: u32 = 3;

// Register offsets within PCI resource on BAR1.
pub const PVRDMA_REG_VERSION: u64 = 0;
pub const PVRDMA_REG_DSRLOW: u64 = 4;
pub const PVRDMA_REG_DSRHIGH: u64 = 8;
pub const PVRDMA_REG_CTL: u64 = 12;
pub const PVRDMA_REG_REQUEST: u64 = 16;
pub const PVRDMA_REG_ERR: u64 = 20;
pub const PVRDMA_REG_ICR: u64 = 24;
pub const PVRDMA_REG_IMR: u64 = 28;
pub const PVRDMA_REG_MACL: u64 = 32;
pub const PVRDMA_REG_MACH: u64 = 36;

// Object flags

/// Armed for solicited-only
pub const PVRDMA_CQ_FLAG_ARMED_SOL: u32 = bit!(0);
/// Armed
pub const PVRDMA_CQ_FLAG_ARMED: u32 = bit!(1);
/// DMA region
pub const PVRDMA_MR_FLAG_DMA: u32 = bit!(0);
/// Fast reg memory region
pub const PVRDMA_MR_FLAG_FRMR: u32 = bit!(1);

// Base Memory Management Extension flags to support Fast Reg Memory Regions and
// Fast Reg Work Requests. Each flag represents a verb operation and we must
// support all of them to qualify for the BMME device cap.

/// Local Invalidate
pub const PVRDMA_BMME_FLAG_LOCAL_INV: u32 = bit!(0);
/// Remote Invalidate
pub const PVRDMA_BMME_FLAG_REMOTE_INV: u32 = bit!(1);
/// Fast Reg Work Request
pub const PVRDMA_BMME_FLAG_FAST_REG_WR: u32 = bit!(2);

// GID types. The interpretation of the gid_types bit field in the device
// capabilities will depend on the device mode. For now, the device only
// supports RoCE as mode, so only the different GID types for RoCE are
// defined.

pub const PVRDMA_GID_TYPE_FLAG_ROCE_V1: u32 = bit!(0);
pub const PVRDMA_GID_TYPE_FLAG_ROCE_V2: u32 = bit!(1);

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_pci_resource {
    /// BAR0: MSI-X, MMIO
    PVRDMA_PCI_RESOURCE_MSIX = 0,
    /// BAR1: Registers, MMIO
    PVRDMA_PCI_RESOURCE_REG = 1,
    /// BAR2: UAR pages, MMIO, 64-bit
    PVRDMA_PCI_RESOURCE_UAR = 2,
    /// Last
    PVRDMA_PCI_RESOURCE_LAST = 3,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_device_ctl {
    /// Activate device
    PVRDMA_DEVICE_CTL_ACTIVATE = 0,
    /// Unquiesce device
    PVRDMA_DEVICE_CTL_UNQUIESCE = 1,
    /// Reset device
    PVRDMA_DEVICE_CTL_RESET = 2,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_intr_vector {
    /// Command response
    PVRDMA_INTR_VECTOR_RESPONSE = 0,
    /// Async events
    PVRDMA_INTR_VECTOR_ASYNC = 1,
    /// CQ notification
    PVRDMA_INTR_VECTOR_CQ = 2,
    // Additional CQ notification vectors
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_intr_cause {
    PVRDMA_INTR_CAUSE_RESPONSE = (1 << pvrdma_intr_vector::PVRDMA_INTR_VECTOR_RESPONSE as u32),
    PVRDMA_INTR_CAUSE_ASYNC = (1 << pvrdma_intr_vector::PVRDMA_INTR_VECTOR_ASYNC as u32),
    PVRDMA_INTR_CAUSE_CQ = (1 << pvrdma_intr_vector::PVRDMA_INTR_VECTOR_CQ as u32),
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_gos_bits {
    // Unknown
    PVRDMA_GOS_BITS_UNK = 0,
    /// 32-bit
    PVRDMA_GOS_BITS_32 = 1,
    /// 64-bit
    PVRDMA_GOS_BITS_64 = 2,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_gos_type {
    /// Unknown
    PVRDMA_GOS_TYPE_UNK = 0,
    /// Linux
    PVRDMA_GOS_TYPE_LINUX = 1,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_device_mode {
    /// RoCE
    PVRDMA_DEVICE_MODE_ROCE = 0,
    /// iWarp
    PVRDMA_DEVICE_MODE_IWARP = 1,
    /// InfiniBand
    PVRDMA_DEVICE_MODE_IB = 2,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_gos_info {
    pub _bitfield_align_1: [u16; 0],
    pub _bitfield_1: __BindgenBitfieldUnit<[u8; 4usize]>,
    pub pad: u32,
}

impl pvrdma_gos_info {
    /// W: PVRDMA_GOS_BITS_
    #[inline]
    pub fn gos_bits(&self) -> pvrdma_gos_bits {
        unsafe { ::core::mem::transmute(self._bitfield_1.get(0usize, 2u8) as u32) }
    }
    #[inline]
    pub fn set_gos_bits(&mut self, val: pvrdma_gos_bits) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set(0usize, 2u8, val as u64)
        }
    }

    /// W: PVRDMA_GOS_TYPE_
    #[inline]
    pub fn gos_type(&self) -> pvrdma_gos_type {
        unsafe { ::core::mem::transmute(self._bitfield_1.get(2usize, 4u8) as u32) }
    }

    #[inline]
    pub fn set_gos_type(&mut self, val: pvrdma_gos_type) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set(2usize, 4u8, val as u64)
        }
    }

    /// Guest OS version
    #[inline]
    pub fn gos_ver(&self) -> u32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get(6usize, 16u8) as u32) }
    }

    #[inline]
    pub fn set_gos_ver(&mut self, val: u32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set(6usize, 16u8, val as u64)
        }
    }

    /// W: Other
    #[inline]
    pub fn gos_misc(&self) -> u32 {
        unsafe { ::core::mem::transmute(self._bitfield_1.get(22usize, 10u8) as u32) }
    }

    #[inline]
    pub fn set_gos_misc(&mut self, val: u32) {
        unsafe {
            let val: u32 = ::core::mem::transmute(val);
            self._bitfield_1.set(22usize, 10u8, val as u64)
        }
    }

    #[inline]
    pub fn new(
        gos_bits: pvrdma_gos_bits,
        gos_type: pvrdma_gos_type,
        gos_ver: u32,
        gos_misc: u32,
    ) -> Self {
        let mut __bindgen_bitfield_unit: __BindgenBitfieldUnit<[u8; 4usize]> = Default::default();
        __bindgen_bitfield_unit.set(0usize, 2u8, {
            let gos_bits: u32 = unsafe { ::core::mem::transmute(gos_bits) };
            gos_bits as u64
        });
        __bindgen_bitfield_unit.set(2usize, 4u8, {
            let gos_type: u32 = unsafe { ::core::mem::transmute(gos_type) };
            gos_type as u64
        });
        __bindgen_bitfield_unit.set(6usize, 16u8, {
            let gos_ver: u32 = unsafe { ::core::mem::transmute(gos_ver) };
            gos_ver as u64
        });
        __bindgen_bitfield_unit.set(22usize, 10u8, {
            let gos_misc: u32 = unsafe { ::core::mem::transmute(gos_misc) };
            gos_misc as u64
        });

        pvrdma_gos_info {
            _bitfield_align_1: [0; 0],
            _bitfield_1: __bindgen_bitfield_unit,
            pad: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_device_caps {
    /// R: Query device
    pub fw_ver: u64,
    pub node_guid: be64,
    pub sys_image_guid: be64,
    pub max_mr_size: u64,
    pub page_size_cap: u64,
    /// EX verbs
    pub atomic_arg_sizes: u64,
    /// EX verbs
    pub ex_comp_mask: u32,
    /// EX verbs
    pub device_cap_flags2: u32,
    /// EX verbs
    pub max_fa_bit_boundary: u32,
    /// EX verbs
    pub log_max_atomic_inline_arg: u32,
    /// EX verbs
    pub vendor_id: u32,
    /// EX verbs
    pub vendor_part_id: u32,
    pub hw_ver: u32,
    pub max_qp: u32,
    pub max_qp_wr: u32,
    pub device_cap_flags: u32,
    pub max_sge: u32,
    pub max_sge_rd: u32,
    pub max_cq: u32,
    pub max_cqe: u32,
    pub max_mr: u32,
    pub max_pd: u32,
    pub max_qp_rd_atom: u32,
    pub max_ee_rd_atom: u32,
    pub max_res_rd_atom: u32,
    pub max_qp_init_rd_atom: u32,
    pub max_ee_init_rd_atom: u32,
    pub max_ee: u32,
    pub max_rdd: u32,
    pub max_mw: u32,
    pub max_raw_ipv6_qp: u32,
    pub max_raw_ethy_qp: u32,
    pub max_mcast_grp: u32,
    pub max_mcast_qp_attach: u32,
    pub max_total_mcast_qp_attach: u32,
    pub max_ah: u32,
    pub max_fmr: u32,
    pub max_map_per_fmr: u32,
    pub max_srq: u32,
    pub max_srq_wr: u32,
    pub max_srq_sge: u32,
    pub max_uar: u32,
    pub gid_tbl_len: u32,
    pub max_pkeys: u16,
    pub local_ca_ack_delay: u8,
    pub phys_port_cnt: u8,
    /// PVRDMA_DEVICE_MODE_
    pub mode: u8,
    /// PVRDMA_ATOMIC_OP_* bits
    pub atomic_ops: u8,
    /// FRWR Mem Mgmt Extensions
    pub bmme_flags: u8,
    /// PVRDMA_GID_TYPE_FLAG_
    pub gid_types: u8,
    pub max_fast_reg_page_list_len: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_ring_page_info {
    /// Num pages incl. header
    pub num_pages: u32,
    /// Reserved
    pub reserved: u32,
    /// Page directory PA
    pub pdir_dma: u64,
}

impl pvrdma_ring_page_info {
    pub fn new(num_pages: u32, pdir_dma: IOAddr) -> Self {
        Self {
            num_pages,
            reserved: 0,
            pdir_dma: pdir_dma.as_u64(),
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct pvrdma_device_shared_region {
    /// W: Driver version
    pub driver_version: u32,
    /// Pad to 8-byte align
    pub pad: u32,
    /// Guest OS information
    pub gos_info: pvrdma_gos_info,
    /// W: Command slot address
    pub cmd_slot_dma: u64,
    /// W: Response slot address
    pub resp_slot_dma: u64,
    /// W: Async ring page info
    pub async_ring_pages: pvrdma_ring_page_info,
    /// W: CQ ring page info
    pub cq_ring_pages: pvrdma_ring_page_info,
    /// W: UAR page frame (32bit val in case driver version is less than
    /// PVRDMA_PPN64_VERSION)
    pub uar_pfn: u64,
    /// Device capabilities
    pub caps: pvrdma_device_caps,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_eqe_type {
    PVRDMA_EVENT_CQ_ERR = 0,
    PVRDMA_EVENT_QP_FATAL = 1,
    PVRDMA_EVENT_QP_REQ_ERR = 2,
    PVRDMA_EVENT_QP_ACCESS_ERR = 3,
    PVRDMA_EVENT_COMM_EST = 4,
    PVRDMA_EVENT_SQ_DRAINED = 5,
    PVRDMA_EVENT_PATH_MIG = 6,
    PVRDMA_EVENT_PATH_MIG_ERR = 7,
    PVRDMA_EVENT_DEVICE_FATAL = 8,
    PVRDMA_EVENT_PORT_ACTIVE = 9,
    PVRDMA_EVENT_PORT_ERR = 10,
    PVRDMA_EVENT_LID_CHANGE = 11,
    PVRDMA_EVENT_PKEY_CHANGE = 12,
    PVRDMA_EVENT_SM_CHANGE = 13,
    PVRDMA_EVENT_SRQ_ERR = 14,
    PVRDMA_EVENT_SRQ_LIMIT_REACHED = 15,
    PVRDMA_EVENT_QP_LAST_WQE_REACHED = 16,
    PVRDMA_EVENT_CLIENT_REREGISTER = 17,
    PVRDMA_EVENT_GID_CHANGE = 18,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_eqe {
    pub type_: u32,
    pub info: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cqne {
    pub info: u32,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_cmd_typ {
    PVRDMA_CMD_FIRST = 0,
    PVRDMA_CMD_QUERY_PKEY = 1,
    PVRDMA_CMD_CREATE_PD = 2,
    PVRDMA_CMD_DESTROY_PD = 3,
    PVRDMA_CMD_CREATE_MR = 4,
    PVRDMA_CMD_DESTROY_MR = 5,
    PVRDMA_CMD_CREATE_CQ = 6,
    PVRDMA_CMD_RESIZE_CQ = 7,
    PVRDMA_CMD_DESTROY_CQ = 8,
    PVRDMA_CMD_CREATE_QP = 9,
    PVRDMA_CMD_MODIFY_QP = 10,
    PVRDMA_CMD_QUERY_QP = 11,
    PVRDMA_CMD_DESTROY_QP = 12,
    PVRDMA_CMD_CREATE_UC = 13,
    PVRDMA_CMD_DESTROY_UC = 14,
    PVRDMA_CMD_CREATE_BIND = 15,
    PVRDMA_CMD_DESTROY_BIND = 16,
    PVRDMA_CMD_CREATE_SRQ = 17,
    PVRDMA_CMD_MODIFY_SRQ = 18,
    PVRDMA_CMD_QUERY_SRQ = 19,
    PVRDMA_CMD_DESTROY_SRQ = 20,
    PVRDMA_CMD_MAX = 21,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum pvrdma_resp_cmd_typ {
    PVRDMA_CMD_FIRST_RESP = -2147483648,
    PVRDMA_CMD_QUERY_PKEY_RESP = -2147483647,
    PVRDMA_CMD_CREATE_PD_RESP = -2147483646,
    PVRDMA_CMD_DESTROY_PD_RESP_NOOP = -2147483645,
    PVRDMA_CMD_CREATE_MR_RESP = -2147483644,
    PVRDMA_CMD_DESTROY_MR_RESP_NOOP = -2147483643,
    PVRDMA_CMD_CREATE_CQ_RESP = -2147483642,
    PVRDMA_CMD_RESIZE_CQ_RESP = -2147483641,
    PVRDMA_CMD_DESTROY_CQ_RESP_NOOP = -2147483640,
    PVRDMA_CMD_CREATE_QP_RESP = -2147483639,
    PVRDMA_CMD_MODIFY_QP_RESP = -2147483638,
    PVRDMA_CMD_QUERY_QP_RESP = -2147483637,
    PVRDMA_CMD_DESTROY_QP_RESP = -2147483636,
    PVRDMA_CMD_CREATE_UC_RESP = -2147483635,
    PVRDMA_CMD_DESTROY_UC_RESP_NOOP = -2147483634,
    PVRDMA_CMD_CREATE_BIND_RESP_NOOP = -2147483633,
    PVRDMA_CMD_DESTROY_BIND_RESP_NOOP = -2147483632,
    PVRDMA_CMD_CREATE_SRQ_RESP = -2147483631,
    PVRDMA_CMD_MODIFY_SRQ_RESP = -2147483630,
    PVRDMA_CMD_QUERY_SRQ_RESP = -2147483629,
    PVRDMA_CMD_DESTROY_SRQ_RESP = -2147483628,
    PVRDMA_CMD_MAX_RESP = -2147483627,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_hdr {
    pub response: u64,
    pub cmd: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_resp_hdr {
    pub response: u64,
    pub ack: u32,
    pub err: u8,
    pub reserved: [u8; 3usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_port {
    pub hdr: pvrdma_cmd_hdr,
    pub port_num: u8,
    pub reserved: [u8; 7usize],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_port_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub attrs: pvrdma_port_attr,
}
impl Default for pvrdma_cmd_query_port_resp {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_pkey {
    pub hdr: pvrdma_cmd_hdr,
    pub port_num: u8,
    pub index: u8,
    pub reserved: [u8; 6usize],
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_pkey_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub pkey: u16,
    pub reserved: [u8; 6usize],
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct pvrdma_cmd_create_uc {
    pub hdr: pvrdma_cmd_hdr,
    pub __bindgen_anon_1: pvrdma_cmd_create_uc__bindgen_ty_1,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union pvrdma_cmd_create_uc__bindgen_ty_1 {
    pub pfn: u32,
    pub pfn64: u64,
}

impl Default for pvrdma_cmd_create_uc__bindgen_ty_1 {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}

impl Default for pvrdma_cmd_create_uc {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_uc_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub ctx_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_uc {
    pub hdr: pvrdma_cmd_hdr,
    pub ctx_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_pd {
    pub hdr: pvrdma_cmd_hdr,
    pub ctx_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_pd_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub pd_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_pd {
    pub hdr: pvrdma_cmd_hdr,
    pub pd_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_mr {
    pub hdr: pvrdma_cmd_hdr,
    pub start: u64,
    pub length: u64,
    pub pdir_dma: u64,
    pub pd_handle: u32,
    pub access_flags: u32,
    pub flags: u32,
    pub nchunks: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_mr_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub mr_handle: u32,
    pub lkey: u32,
    pub rkey: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_mr {
    pub hdr: pvrdma_cmd_hdr,
    pub mr_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_cq {
    pub hdr: pvrdma_cmd_hdr,
    pub pdir_dma: u64,
    pub ctx_handle: u32,
    pub cqe: u32,
    pub nchunks: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_cq_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub cq_handle: u32,
    pub cqe: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_resize_cq {
    pub hdr: pvrdma_cmd_hdr,
    pub cq_handle: u32,
    pub cqe: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_resize_cq_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub cqe: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_cq {
    pub hdr: pvrdma_cmd_hdr,
    pub cq_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_srq {
    pub hdr: pvrdma_cmd_hdr,
    pub pdir_dma: u64,
    pub pd_handle: u32,
    pub nchunks: u32,
    pub attrs: pvrdma_srq_attr,
    pub srq_type: u8,
    pub reserved: [u8; 7usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_srq_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub srqn: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_modify_srq {
    pub hdr: pvrdma_cmd_hdr,
    pub srq_handle: u32,
    pub attr_mask: u32,
    pub attrs: pvrdma_srq_attr,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_srq {
    pub hdr: pvrdma_cmd_hdr,
    pub srq_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_srq_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub attrs: pvrdma_srq_attr,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_srq {
    pub hdr: pvrdma_cmd_hdr,
    pub srq_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_qp {
    pub hdr: pvrdma_cmd_hdr,
    pub pdir_dma: u64,
    pub pd_handle: u32,
    pub send_cq_handle: u32,
    pub recv_cq_handle: u32,
    pub srq_handle: u32,
    pub max_send_wr: u32,
    pub max_recv_wr: u32,
    pub max_send_sge: u32,
    pub max_recv_sge: u32,
    pub max_inline_data: u32,
    pub lkey: u32,
    pub access_flags: u32,
    pub total_chunks: u16,
    pub send_chunks: u16,
    pub max_atomic_arg: u16,
    pub sq_sig_all: u8,
    pub qp_type: u8,
    pub is_srq: u8,
    pub reserved: [u8; 3usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_qp_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub qpn: u32,
    pub max_send_wr: u32,
    pub max_recv_wr: u32,
    pub max_send_sge: u32,
    pub max_recv_sge: u32,
    pub max_inline_data: u32,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_qp_resp_v2 {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub qpn: u32,
    pub qp_handle: u32,
    pub max_send_wr: u32,
    pub max_recv_wr: u32,
    pub max_send_sge: u32,
    pub max_recv_sge: u32,
    pub max_inline_data: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct pvrdma_cmd_modify_qp {
    pub hdr: pvrdma_cmd_hdr,
    pub qp_handle: u32,
    pub attr_mask: u32,
    pub attrs: pvrdma_qp_attr,
}

impl Default for pvrdma_cmd_modify_qp {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_query_qp {
    pub hdr: pvrdma_cmd_hdr,
    pub qp_handle: u32,
    pub attr_mask: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct pvrdma_cmd_query_qp_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub attrs: pvrdma_qp_attr,
}

impl Default for pvrdma_cmd_query_qp_resp {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_qp {
    pub hdr: pvrdma_cmd_hdr,
    pub qp_handle: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_qp_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub events_reported: u32,
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_create_bind {
    pub hdr: pvrdma_cmd_hdr,
    pub mtu: u32,
    pub vlan: u32,
    pub index: u32,
    pub new_gid: [u8; 16usize],
    pub gid_type: u8,
    pub reserved: [u8; 3usize],
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct pvrdma_cmd_destroy_bind {
    pub hdr: pvrdma_cmd_hdr,
    pub index: u32,
    pub dest_gid: [u8; 16usize],
    pub reserved: [u8; 4usize],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union pvrdma_cmd_req {
    pub hdr: pvrdma_cmd_hdr,
    pub query_port: pvrdma_cmd_query_port,
    pub query_pkey: pvrdma_cmd_query_pkey,
    pub create_uc: pvrdma_cmd_create_uc,
    pub destroy_uc: pvrdma_cmd_destroy_uc,
    pub create_pd: pvrdma_cmd_create_pd,
    pub destroy_pd: pvrdma_cmd_destroy_pd,
    pub create_mr: pvrdma_cmd_create_mr,
    pub destroy_mr: pvrdma_cmd_destroy_mr,
    pub create_cq: pvrdma_cmd_create_cq,
    pub resize_cq: pvrdma_cmd_resize_cq,
    pub destroy_cq: pvrdma_cmd_destroy_cq,
    pub create_qp: pvrdma_cmd_create_qp,
    pub modify_qp: pvrdma_cmd_modify_qp,
    pub query_qp: pvrdma_cmd_query_qp,
    pub destroy_qp: pvrdma_cmd_destroy_qp,
    pub create_bind: pvrdma_cmd_create_bind,
    pub destroy_bind: pvrdma_cmd_destroy_bind,
    pub create_srq: pvrdma_cmd_create_srq,
    pub modify_srq: pvrdma_cmd_modify_srq,
    pub query_srq: pvrdma_cmd_query_srq,
    pub destroy_srq: pvrdma_cmd_destroy_srq,
}

impl Default for pvrdma_cmd_req {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union pvrdma_cmd_resp {
    pub hdr: pvrdma_cmd_resp_hdr,
    pub query_port_resp: pvrdma_cmd_query_port_resp,
    pub query_pkey_resp: pvrdma_cmd_query_pkey_resp,
    pub create_uc_resp: pvrdma_cmd_create_uc_resp,
    pub create_pd_resp: pvrdma_cmd_create_pd_resp,
    pub create_mr_resp: pvrdma_cmd_create_mr_resp,
    pub create_cq_resp: pvrdma_cmd_create_cq_resp,
    pub resize_cq_resp: pvrdma_cmd_resize_cq_resp,
    pub create_qp_resp: pvrdma_cmd_create_qp_resp,
    pub create_qp_resp_v2: pvrdma_cmd_create_qp_resp_v2,
    pub query_qp_resp: pvrdma_cmd_query_qp_resp,
    pub destroy_qp_resp: pvrdma_cmd_destroy_qp_resp,
    pub create_srq_resp: pvrdma_cmd_create_srq_resp,
    pub query_srq_resp: pvrdma_cmd_query_srq_resp,
}

impl Default for pvrdma_cmd_resp {
    fn default() -> Self {
        unsafe { ::core::mem::zeroed() }
    }
}
