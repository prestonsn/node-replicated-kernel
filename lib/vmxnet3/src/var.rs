/* automatically generated by rust-bindgen 0.57.0 */
#![allow(non_camel_case_types)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{collections::VecDeque, format};
use core::intrinsics::unlikely;
use core::{convert::TryInto, ptr};

use driverkit::{
    devq::{DevQueue, DevQueueError},
    iomem::{IOBuf, IOBufChain},
    net::csum::*,
    net::rss::*,
};
use log::{debug, info};

use x86::current::paging::{PAddr, VAddr};

use crate::pci::{BarAccess, BarIO, DmaObject, KERNEL_BASE};
use crate::reg::*;
use crate::vmx::{Barrier, RxQueueId, TxQueueId, VMXNet3, VMXNet3Error};
use crate::BoundedUSize;

pub type c_uint = u32;
pub type c_int = i32;
pub type u_int = c_uint;

pub const ETHER_ADDR_LEN: u32 = 6;

pub const VMXNET3_DEF_RX_QUEUES: u32 = 8;
pub const VMXNET3_DEF_TX_QUEUES: u32 = 8;
pub const VMXNET3_RXRINGS_PERQ: u32 = 2;
pub const VMXNET3_DEF_TX_NDESC: usize = 512;
pub const VMXNET3_MAX_TX_NDESC: usize = 4096;
pub const VMXNET3_MIN_TX_NDESC: usize = 32;
pub const VMXNET3_MASK_TX_NDESC: usize = 31;
pub const VMXNET3_DEF_RX_NDESC: usize = 512;
pub const VMXNET3_MAX_RX_NDESC: usize = 2048;
pub const VMXNET3_MIN_RX_NDESC: usize = 32;
pub const VMXNET3_MASK_RX_NDESC: usize = 31;
pub const VMXNET3_MAX_TX_NCOMPDESC: usize = 4096;
pub const VMXNET3_MAX_RX_NCOMPDESC: usize = 4096;
pub const VMXNET3_FLAG_RSS: u32 = 2;
pub const VMXNET3_FLAG_SOFT_RSS: u32 = 4;
pub const VMXNET3_DRIVER_VERSION: u32 = 65536;
pub const VMXNET3_TX_MAXSEGS: usize = 32;
pub const VMXNET3_TX_MAXSEGSIZE: usize = 16383;
pub const VMXNET3_RX_MAXSEGSIZE: usize = 16383;
pub const VMXNET3_MULTICAST_MAX: usize = 32;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct iflib_dma_info {
    pub p: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct if_irq {
    pub p: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct devicet {
    pub p: u8,
}

pub type device_t = devicet;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct if_ctx {
    pub p: u8,
}

pub type if_ctx_t = if_ctx;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct if_shared_ctx {
    pub p: u8,
}

pub type if_shared_ctx_t = if_shared_ctx;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct if_softc_ctx {
    pub p: u8,
}

pub type if_softc_ctx_t = if_softc_ctx;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct bus_space_handle {
    pub p: u8,
}

pub type bus_space_handle_t = bus_space_handle;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct bus_space_tag {
    pub p: u8,
}

pub type bus_space_tag_t = bus_space_tag;
pub type bus_addr_t = u64;

#[repr(C)]
#[derive(Debug)]
pub struct vmxnet3_txring {
    pub vxtxr_next: u_int,
    //pub vxtxr_ndesc: u_int, //TODO: is this save to remove?
    pub vxtxr_gen: u32,
    pub vxtxr_txd: Vec<vmxnet3_txdesc>,
}

impl vmxnet3_txring {
    fn new(vxtxr_ndesc: usize) -> Result<Self, VMXNet3Error> {
        let mut vxtxr_txd: Vec<vmxnet3_txdesc> = Vec::new();
        vxtxr_txd.try_reserve_exact(vxtxr_ndesc)?;

        for _i in 0..vxtxr_ndesc {
            vxtxr_txd.push(vmxnet3_txdesc::default());
        }

        Ok(vmxnet3_txring {
            vxtxr_next: 0,
            vxtxr_gen: VMXNET3_INIT_GEN,
            vxtxr_txd,
        })
    }

    pub(crate) fn vxtxr_ndesc(&self) -> usize {
        self.vxtxr_txd.len()
    }
}

impl DmaObject for vmxnet3_txring {
    fn paddr(&self) -> PAddr {
        PAddr::from(self.vxtxr_txd.as_ptr() as u64 - KERNEL_BASE)
    }

    fn vaddr(&self) -> VAddr {
        VAddr::from(self.vxtxr_txd.as_ptr() as u64)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct vmxnet3_rxring {
    pub vxrxr_rxd: Vec<vmxnet3_rxdesc>,
    pub vxrxr_gen: u32,
    pub vxrxr_desc_skips: u64,
    pub vxrxr_refill_start: usize,
}

impl vmxnet3_rxring {
    fn new(vxrxr_ndesc: usize) -> Result<Self, VMXNet3Error> {
        let mut vxrxr_rxd: Vec<vmxnet3_rxdesc> = Vec::new();
        vxrxr_rxd.try_reserve_exact(vxrxr_ndesc)?;
        for i in 0..vxrxr_ndesc {
            vxrxr_rxd.push(vmxnet3_rxdesc::default());
        }

        Ok(vmxnet3_rxring {
            vxrxr_rxd,
            vxrxr_gen: VMXNET3_INIT_GEN,
            vxrxr_desc_skips: 0,
            vxrxr_refill_start: 0,
        })
    }

    pub fn vxrxr_ndesc(&self) -> usize {
        self.vxrxr_rxd.len()
    }
}

impl DmaObject for vmxnet3_rxring {
    fn paddr(&self) -> PAddr {
        if self.vxrxr_ndesc() == 0 {
            return PAddr::zero();
        }
        PAddr::from(self.vxrxr_rxd.as_ptr() as u64 - KERNEL_BASE)
    }

    fn vaddr(&self) -> VAddr {
        VAddr::from(self.vxrxr_rxd.as_ptr() as u64)
    }
}

/// A vector of either Rx or Tx completion descriptors.
pub enum CompRingBuf {
    TxCd(Vec<vmxnet3_txcompdesc>),
    RxCd(Vec<vmxnet3_rxcompdesc>),
}

/// A completion ring that maintains some statistics about paket errors
/// and zero length packets encountered.
#[repr(C)]
pub struct vmxnet3_comp_ring {
    pub vxcr: CompRingBuf,
    pub vxcr_next: usize,
    pub vxcr_gen: u32,
    pub vxcr_zero_length: u64,
    pub vxcr_pkt_errors: u64,
}

impl vmxnet3_comp_ring {
    pub(crate) fn new_rx(ndesc: usize) -> Result<Self, VMXNet3Error> {
        let mut vxcr = Vec::new();
        vxcr.try_reserve_exact(ndesc)?;

        for i in 0..ndesc {
            vxcr.push(vmxnet3_rxcompdesc::default());
        }

        Ok(vmxnet3_comp_ring {
            vxcr: CompRingBuf::RxCd(vxcr),
            vxcr_next: 0,
            vxcr_gen: VMXNET3_INIT_GEN,
            vxcr_zero_length: 0,
            vxcr_pkt_errors: 0,
        })
    }

    pub(crate) fn new_tx(ndesc: usize) -> Result<Self, VMXNet3Error> {
        let mut vxcr = Vec::new();
        vxcr.try_reserve_exact(ndesc)?;
        for i in 0..ndesc {
            vxcr.push(vmxnet3_txcompdesc::default());
        }

        Ok(vmxnet3_comp_ring {
            vxcr: CompRingBuf::TxCd(vxcr),
            vxcr_next: 0,
            vxcr_gen: VMXNET3_INIT_GEN,
            vxcr_zero_length: 0,
            vxcr_pkt_errors: 0,
        })
    }

    pub(crate) fn vxcr_ndesc(&self) -> usize {
        match &self.vxcr {
            CompRingBuf::RxCd(buf) => buf.len(),
            CompRingBuf::TxCd(buf) => buf.len(),
        }
    }
}

impl DmaObject for vmxnet3_comp_ring {
    fn paddr(&self) -> PAddr {
        match &self.vxcr {
            CompRingBuf::RxCd(buf) => PAddr::from(buf.as_ptr() as u64 - KERNEL_BASE),
            CompRingBuf::TxCd(buf) => PAddr::from(buf.as_ptr() as u64 - KERNEL_BASE),
        }
    }

    fn vaddr(&self) -> VAddr {
        match &self.vxcr {
            CompRingBuf::RxCd(buf) => VAddr::from(buf.as_ptr() as u64),
            CompRingBuf::TxCd(buf) => VAddr::from(buf.as_ptr() as u64),
        }
    }
}

#[repr(C, align(64))]
pub struct TxQueue {
    pub(crate) vxtxq_id: TxQueueId,
    pub(crate) vxtxq_last_flush: c_int,
    pub(crate) vxtxq_intr_idx: c_int,
    pub(crate) vxtxq_cmd_ring: vmxnet3_txring,
    pub(crate) vxtxq_comp_ring: vmxnet3_comp_ring,
    /// Let's us access device' PCI registers
    pci: BarAccess,
    // tail and head are pointers into the buffer. Tail always points
    // to the first element that could be read, Head always points
    // to where data should be written.
    // If tail == head the buffer is empty. The length of the ringbuffer
    // is defined as the distance between the two.
    /// Stores advanced pidx between enqueue() and flush()
    pidx_tail: usize,
    /// Current index into descriptors
    pidx_head: usize,
    /// Holding area for chains waiting to be sent by NIC.
    ///
    /// Format is (pidx_of_last_segment, IOBufChain)
    inflight_chains: VecDeque<(usize, IOBufChain)>,
    /// Holding area for IOBufChain that are waiting to be dequeued again
    processed_chains: VecDeque<IOBufChain>,
}

impl TxQueue {
    pub(crate) fn new(
        vxtxq_id: TxQueueId,
        ndesc: usize,
        pci: BarAccess,
    ) -> Result<Self, VMXNet3Error> {
        let vxtxr_ndesc = BoundedUSize::<VMXNET3_MIN_TX_NDESC, VMXNET3_MAX_TX_NDESC>::new(ndesc);

        let mut inflight_chains = VecDeque::new();
        inflight_chains.try_reserve_exact(*vxtxr_ndesc)?;

        let mut processed_chains = VecDeque::new();
        processed_chains.try_reserve_exact(*vxtxr_ndesc)?;

        // Enforce that the transmit completion queue descriptor count is
        // the same as the transmit command queue descriptor count.
        Ok(TxQueue {
            vxtxq_id,
            vxtxq_last_flush: -1,
            vxtxq_intr_idx: 0,
            vxtxq_cmd_ring: vmxnet3_txring::new(*vxtxr_ndesc)?,
            vxtxq_comp_ring: vmxnet3_comp_ring::new_tx(*vxtxr_ndesc)?,
            pci,
            pidx_tail: 0,
            pidx_head: 0,
            inflight_chains,
            processed_chains,
        })
    }

    pub fn len(&self) -> usize {
        let size = self.vxtxq_cmd_ring.vxtxr_ndesc();
        debug_assert!(size.is_power_of_two());

        (self.pidx_head.wrapping_sub(self.pidx_tail)) & (size - 1)
    }

    pub fn capacity(&self) -> usize {
        self.vxtxq_cmd_ring.vxtxr_ndesc()
    }

    pub fn is_full(&self) -> bool {
        self.capacity() - self.len() == 1
    }

    pub fn is_empty(&self) -> bool {
        self.pidx_tail == self.pidx_head
    }

    pub fn vxtxq_name(&self) -> String {
        format!("tx-{}", self.vxtxq_id)
    }

    /*
    pub fn vxtxq_ts(&self) -> *mut vmxnet3_txq_shared {
        unimplemented!("get this through vmxnet3 struct?")
    } */
}

impl DmaObject for TxQueue {}

impl DevQueue for TxQueue {
    fn enqueue(&mut self, chain: IOBufChain) -> Result<(), IOBufChain> {
        assert!(
            chain.segments.len() <= VMXNET3_TX_MAXSEGS,
            "vmxnet3: Packet with too many segments"
        );

        if self.capacity() - self.len() < chain.segments.len() {
            // We don't bother trying to enqueue a partial packet
            return Err(chain);
        }

        let txr = &mut self.vxtxq_cmd_ring;
        let old_head = self.pidx_head;
        let mut gen = txr.vxtxr_gen ^ 1; /* Owned by cpu (yet) */
        let ndesc = txr.vxtxr_ndesc();

        let mut segments = chain.segments.iter().peekable();
        while let Some(seg) = segments.next() {
            // is_full() (inlined for borrow checking)
            if ndesc - ((self.pidx_head.wrapping_sub(self.pidx_tail)) & (ndesc - 1)) == 1 {
                panic!("ring is full, but we checked this...?");
            }

            let txd = &mut txr.vxtxr_txd[self.pidx_head];
            info!("enq packet at {:#x} len:{}", seg.paddr(), seg.len());

            txd.addr = seg.paddr().as_u64();
            txd.set_len(seg.len().try_into().unwrap());
            txd.set_gen(gen as u32);
            txd.set_dtype(0);
            txd.set_offload_mode(VMXNET3_OM_NONE);
            txd.set_offload_pos(0);
            txd.set_hlen(0);
            txd.set_eop(0);
            txd.set_compreq(0);
            txd.set_vtag_mode(0);
            txd.set_vtag(0);

            self.pidx_head += 1;
            if self.pidx_head == ndesc {
                self.pidx_head = 0;
                txr.vxtxr_gen ^= 1;
            }
            gen = txr.vxtxr_gen;

            // Is this the last segment?
            if segments.peek().is_none() {
                txd.set_eop(1);
                // send an interrupt when this packet is sent
                const IPI_TX_INTR: u32 = 0x1;
                txd.set_compreq(!!(chain.flags & IPI_TX_INTR));
                info!("txt.compreq {:#x}", txd.compreq());
            }
        }

        // Ignore VLAN
        // Ignore TSO and checksum offload

        VMXNet3::barrier(Barrier::Write);

        let sop = &mut txr.vxtxr_txd[old_head];
        sop.set_gen(sop.gen() ^ 1);

        // Add IOBufChain to the back of the holding area:
        self.inflight_chains.push_back((self.pidx_head - 1, chain));

        Ok(())
    }

    /// Flushes packets to device.
    fn flush(&mut self) -> Result<usize, DevQueueError> {
        // Avoid expensive register updates if the flush request is
        // redundant
        if self.vxtxq_last_flush == (self.pidx_head as i32) {
            return Ok(0);
        }
        self.vxtxq_last_flush = self.pidx_head as i32;

        let bar0_txh_offset = |idx: TxQueueId| 0x600 + idx as u64 * 8;
        self.pci
            .write_bar0(bar0_txh_offset(self.vxtxq_id), self.pidx_head as u32);

        Ok(0)
    }

    fn can_enqueue(&self, how_many_seg: usize) -> bool {
        self.capacity() - self.len() > how_many_seg
    }

    fn dequeue(&mut self) -> Result<IOBufChain, DevQueueError> {
        if !self.processed_chains.is_empty() || self.can_dequeue(false) >= 1 {
            debug_assert!(!self.processed_chains.is_empty());
            self.processed_chains
                .pop_front()
                .ok_or(DevQueueError::QueueEmpty)
        } else {
            Err(DevQueueError::QueueEmpty)
        }
    }

    /// Processes the completion queue of the NIC
    /// updates tail pointer accordingly.
    ///
    /// # Arguments
    /// - exact: If true, advances completion queue as much as possible,
    ///  if false, only checks if at least one IOBufChain can be returned.
    ///
    /// # Returns
    /// How many packets have been sent.
    fn can_dequeue(&mut self, exact: bool) -> usize {
        let txc = &mut self.vxtxq_comp_ring;
        let txr = &mut self.vxtxq_cmd_ring;

        // If exact is true, we need to report the number of TX command ring
        // descriptors that have been processed by the device.  If exact is
        // false, we just need to report whether or not at least one TX
        // command ring descriptor has been processed by the device.

        let mut processed = 0;
        loop {
            if let CompRingBuf::TxCd(txcd_arr) = &mut txc.vxcr {
                let txcd = txcd_arr[txc.vxcr_next as usize];
                if txcd.gen() != txc.vxcr_gen {
                    break;
                }

                VMXNet3::barrier(Barrier::Read);

                txc.vxcr_next += 1;
                if txc.vxcr_next == txc.vxcr_ndesc() {
                    txc.vxcr_next = 0;
                    txc.vxcr_gen ^= 1;
                }
                // TODO: Update chain-holder element here
                let (chain_eop_idx, buf_chain) =
                    self.inflight_chains.pop_front().expect("Expected an entry");
                assert_eq!(chain_eop_idx, txcd.eop_idx() as usize);

                self.processed_chains.push_back(buf_chain);
                processed += 1;

                // replaced with pidx_tail:
                // txr.vxtxr_next = (txcd.eop_idx() + 1) % txr.vxtxr_ndesc() as u32;
                self.pidx_tail = (txcd.eop_idx() as usize + 1) % txr.vxtxr_ndesc();

                if !exact {
                    // Stop after one packet
                    break;
                }
            }
        }

        processed
    }
}

#[repr(C, align(64))]
pub struct RxQueue {
    pub vxrxq_id: RxQueueId,
    pub vxrxq_intr_idx: c_int,
    pub vxrxq_irq: if_irq,
    pub vxrxq_cmd_ring: [vmxnet3_rxring; 2usize],
    pub vxrxq_comp_ring: vmxnet3_comp_ring,
    /// Flags from VMX device (for RSS decisions)
    vmx_flags: u32,
    // tail and head are pointers into the buffer. Tail always points
    // to the first element that could be read, Head always points
    // to where data should be written.
    // If tail == head the buffer is empty. The length of the ringbuffer
    // is defined as the distance between the two.
    /// Stores advanced pidx between enqueue() and flush() for vxrxq_cmd_ring[0]
    pidx_tail0: usize,
    /// Current index into descriptors for vxrxq_cmd_ring[0]
    pidx_head0: usize,
    /// Let's us access device' PCI registers
    pci: BarAccess,
    /// Holding area for chains waiting to filled with packet by NIC.
    ///
    /// Format is (pidx_of_last_segment, IOBufChain)
    inflight_chains: VecDeque<(usize, IOBufChain)>,
    /// Holding area for IOBufChain that are waiting to be dequeued
    processed_chains: VecDeque<IOBufChain>,
    /// Current index into completion queue (of vxrxq_comp_ring)
    cqidx: usize,
}

impl RxQueue {
    pub(crate) fn new(
        vxrxq_id: RxQueueId,
        vmx_flags: u32,
        ndesc: usize,
        pci: BarAccess,
    ) -> Result<Self, VMXNet3Error> {
        let vxtxr_ndesc = BoundedUSize::<VMXNET3_MIN_RX_NDESC, VMXNET3_MAX_RX_NDESC>::new(ndesc);

        let mut inflight_chains = VecDeque::new();
        inflight_chains.try_reserve_exact(*vxtxr_ndesc)?;

        let mut processed_chains = VecDeque::new();
        processed_chains.try_reserve_exact(*vxtxr_ndesc)?;

        // Currently only support single receive queue descriptor ring (TODO: If
        // we support for both, make sure to change vxrxq_comp_ring to 2*ndesc)

        Ok(RxQueue {
            vxrxq_id,
            vxrxq_intr_idx: 0,
            vxrxq_irq: Default::default(),
            vxrxq_cmd_ring: [vmxnet3_rxring::new(ndesc)?, vmxnet3_rxring::new(0)?],
            vxrxq_comp_ring: vmxnet3_comp_ring::new_rx(1 * ndesc)?,
            vmx_flags,
            pidx_tail0: 0,
            pidx_head0: 0,
            pci,
            inflight_chains,
            processed_chains,
            cqidx: 0,
        })
    }

    pub fn len(&self) -> usize {
        let size = self.vxrxq_cmd_ring[0].vxrxr_ndesc();
        debug_assert!(size.is_power_of_two());

        (self.pidx_head0.wrapping_sub(self.pidx_tail0)) & (size - 1)
    }

    pub fn capacity(&self) -> usize {
        self.vxrxq_cmd_ring[0].vxrxr_ndesc()
    }

    pub fn is_full(&self) -> bool {
        self.capacity() - self.len() == 1
    }

    pub fn is_empty(&self) -> bool {
        self.pidx_tail0 == self.pidx_head0
    }

    pub fn vxrxq_name(&self) -> String {
        format!("rx-{}", self.vxrxq_id)
    }

    pub fn vxrxq_rs(&self) -> *mut vmxnet3_rxq_shared {
        unimplemented!("get this through vmxnet3 struct?")
    }
}

impl DmaObject for RxQueue {}

impl DevQueue for RxQueue {
    fn enqueue(&mut self, chain: IOBufChain) -> Result<(), IOBufChain> {
        if self.capacity() - self.len() < chain.segments.len() {
            // We don't bother trying to enqueue a partial packet
            return Err(chain);
        }

        assert_eq!(
            chain.segments.len(),
            2,
            "Only support receive packet with one header and one content segment."
        );

        // TODO: Usually we use both rings (to support LRO), then command ring 0
        // is filled with BTYPE_HEAD descriptors, and command ring 1 is filled
        // with BTYPE_BODY descriptors but currently we don't support LRO so we
        // only need a single ring.
        let flid = 0;

        let rxr = &mut self.vxrxq_cmd_ring[flid];
        let ndesc = rxr.vxrxr_ndesc();
        let rxd = &mut rxr.vxrxr_rxd;

        let mut idx = rxr.vxrxr_refill_start;
        let mut i = 0;
        for chain in chain.segments.iter() {
            rxd[idx].addr = chain.paddr().as_u64();
            rxd[idx].set_len(chain.len().try_into().unwrap());
            rxd[idx].set_btype(if i % 2 == 0 {
                VMXNET3_BTYPE_HEAD
            } else {
                VMXNET3_BTYPE_BODY
            });
            rxd[idx].set_gen(rxr.vxrxr_gen);
            debug!("RxRing enqueued {:?}", rxd[idx]);

            i += 1;
            idx += 1;
            if idx == ndesc {
                idx = 0;
                rxr.vxrxr_gen ^= 1;
            }
        }

        rxr.vxrxr_refill_start = idx;
        // TODO: Maybe we just use `vxrxr_refill_start` instead of `pidx_head0`?
        self.pidx_head0 = idx;

        self.inflight_chains
            .push_back((idx - chain.segments.len(), chain));

        Ok(())
    }

    fn flush(&mut self) -> Result<usize, DevQueueError> {
        let flid = 0; // TODO(unsupported): No support to flush the 2nd ring (RXH2)

        let r = if flid == 0 {
            0x800 + (self.vxrxq_id * 8) as u64
        } else {
            0xA00 + (self.vxrxq_id * 8) as u64
        };

        self.pci.write_bar0(r, self.pidx_head0 as u32);
        Ok(1)
    }

    fn can_enqueue(&self, how_many_seg: usize) -> bool {
        self.capacity() - self.len() > how_many_seg
    }

    fn dequeue(&mut self) -> Result<IOBufChain, DevQueueError> {
        if self.can_dequeue(false) == 0 {
            return Err(DevQueueError::QueueEmpty);
        }

        // Get a single packet starting at the given index in the completion
        // queue. That we have been called indicates that
        // vmxnet3_isc_rxd_available() has already verified that either there is
        // a complete packet available starting at the given index, or there are
        // one or more zero length packets starting at the given index followed
        // by a complete packet, so no verification of ownership of the
        // descriptors (and no associated read barrier) is required here.

        let rxc = &mut self.vxrxq_comp_ring;
        if let CompRingBuf::RxCd(ref descs) = rxc.vxcr {
            let rxcd = descs[self.cqidx];
            // Skip zero-length entries
            while rxcd.len() == 0 {
                assert!(
                    rxcd.eop() && rxcd.sop(),
                    "Zero length packet without sop and eop set"
                );
                rxc.vxcr_zero_length += 1;

                self.cqidx += 1;
                if self.cqidx == rxc.vxcr_ndesc() {
                    self.cqidx = 0;
                    rxc.vxcr_gen ^= 1;
                }
            }
            assert!(rxcd.sop(), "expected sop");

            // RSS and flow ID.
            //
            // Types other than M_HASHTYPE_NONE and M_HASHTYPE_OPAQUE_HASH
            // should be used only if the software RSS is enabled and it uses
            // the same algorithm and the hash key as the "hardware".  If the
            // software RSS is not enabled, then it's simply pointless to use
            // those types. If it's enabled but with different parameters, then
            // hash values will not match.

            // TODO(unused): We currently don't care about RSS, but if we
            // eventually do, we need to convey this info to the buf-chain
            let mut flowid = None;
            let mut rsstype = 0;
            #[cfg(feature = "rss")]
            let rss_flag = self.vmx_flags & VMXNET3_FLAG_SOFT_RSS != 0;
            match rxcd.rss_type() {
                #[cfg(feature = "rss")]
                VMXNET3_RCD_RSS_TYPE_NONE if rss_flag => {
                    flowid = Some(self.vxrxq_id);
                    rsstype = M_HASHTYPE_NONE;
                }
                #[cfg(feature = "rss")]
                VMXNET3_RCD_RSS_TYPE_IPV4 if rss_flag => {
                    rsstype = M_HASHTYPE_RSS_IPV4;
                }
                #[cfg(feature = "rss")]
                VMXNET3_RCD_RSS_TYPE_TCPIPV4 if rss_flag => {
                    rsstype = M_HASHTYPE_RSS_TCP_IPV4;
                }
                #[cfg(feature = "rss")]
                VMXNET3_RCD_RSS_TYPE_IPV6 if rss_flag => {
                    rsstype = M_HASHTYPE_RSS_IPV6;
                }
                #[cfg(feature = "rss")]
                VMXNET3_RCD_RSS_TYPE_TCPIPV6 if rss_flag => {
                    rsstype = M_HASHTYPE_RSS_TCP_IPV6;
                }
                VMXNET3_RCD_RSS_TYPE_NONE => {
                    flowid = Some(self.vxrxq_id);
                    rsstype = M_HASHTYPE_NONE;
                }
                _ => {
                    rsstype = M_HASHTYPE_OPAQUE_HASH;
                }
            }

            // The queue numbering scheme used for rxcd->qid is as follows:
            //  - All of the command ring 0s are numbered [0, nrxqsets - 1]
            //  - All of the command ring 1s are numbered [nrxqsets, 2*nrxqsets
            //    - 1]
            //
            // Thus, rxcd->qid less than nrxqsets indicates command ring (and
            // flid) 0, and rxcd->qid greater than or equal to nrxqsets
            // indicates command ring (and flid) 1.

            let (_chain_idx, mut chain) = self
                .inflight_chains
                .pop_front()
                .expect("IOBufChain not available?");
            let mut nfrags: usize = 0;
            let mut total_len = 0;
            let mut rxcd;
            loop {
                rxcd = &descs[self.cqidx];
                assert_eq!(rxcd.gen(), rxc.vxcr_gen, "generation mismatch");

                // TODO: if we were to use use both rxrings:
                // let flid = if rxcd.qid() >= isc_nrxqsets { 1 } else { 0 };
                let flid = 0;
                let rxr = &self.vxrxq_cmd_ring[flid];

                let rxd_idx = rxcd.rxd_idx() as usize;
                info!("rxcd {:?}", rxcd);
                let _rxd = &rxr.vxrxr_rxd[rxd_idx];

                assert!(
                    nfrags < chain.segments.len(),
                    "Don't support unexpected segments (LRO, 2 queue)"
                );
                //chain.segments[nfrags].flid = flid;
                //chain.segments[nfrags].rxd_idx = rxd_idx;

                let rxcd_len = rxcd.len() as usize;
                debug_assert!(rxcd_len <= chain.segments[nfrags].len());
                chain.segments[nfrags].truncate(rxcd_len);
                total_len += rxcd_len;

                nfrags += 1;
                self.cqidx += 1;
                if self.cqidx == rxc.vxcr_ndesc() {
                    self.cqidx = 0;
                    rxc.vxcr_gen ^= 1;
                }

                if rxcd.eop() {
                    break;
                }
            }

            chain.set_meta_data(total_len, nfrags, self.cqidx, flowid, rsstype);

            // If there's an error, the last descriptor in the packet will have
            // the error indicator set.  In this case, set all fragment lengths
            // to zero. This should cause higher-levels to discard the packet,
            // but process all associated descriptors through the refill
            // mechanism.
            debug_assert!(rxcd.eop());
            if unlikely(rxcd.error()) {
                rxc.vxcr_pkt_errors += 1;
                for segment in chain.segments.iter_mut() {
                    segment.truncate(0);
                }
            } else {
                if !rxcd.no_csum() {
                    let mut csum_flags: u32 = 0;
                    if rxcd.ipv4() {
                        csum_flags |= CSUM_IP_CHECKED;
                        if rxcd.ipcsum_ok() {
                            csum_flags = CSUM_IP_VALID;
                        }
                    }
                    if !rxcd.fragment() && (rxcd.tcp() || rxcd.udp()) {
                        csum_flags |= CSUM_L4_CALC;
                        if rxcd.csum_ok() {
                            csum_flags |= CSUM_L4_VALID;
                            chain.csum_data = 0xffff;
                        }
                    }
                    chain.csum_flags = csum_flags;
                }

                if rxcd.vlan() {
                    chain.vtag = Some(rxcd.vtag());
                }
            }

            Ok(chain)
        } else {
            unreachable!("type error for desc");
        }
    }

    fn can_dequeue(&mut self, exact: bool) -> usize {
        // Start from current self.cqidx
        let mut idx = self.cqidx;
        let budget = if exact { VMXNET3_MAX_RX_NDESC } else { 1 };

        let rxc = &mut self.vxrxq_comp_ring;
        let mut available = 0; // Completed descriptors
        let mut expect_sop = true;

        loop {
            if let CompRingBuf::RxCd(ref descs) = rxc.vxcr {
                let rxcd = descs[idx];
                if rxcd.gen() != rxc.vxcr_gen {
                    break;
                }
                VMXNet3::barrier(Barrier::Read);
                debug!("rxcd is {:?}", rxcd);

                #[cfg(debug_assertions)]
                {
                    // Invariants:
                    if expect_sop {
                        debug_assert!(rxcd.sop(), "expected sop");
                    } else {
                        debug_assert!(!rxcd.sop(), "unexpected sop");
                    }
                    expect_sop = rxcd.eop();
                }

                if rxcd.eop() && rxcd.len() != 0 {
                    available += 1;
                }
                if available > budget {
                    break;
                }
                idx += 1;
                if idx == rxc.vxcr_ndesc() {
                    idx = 0;
                    rxc.vxcr_gen ^= 1;
                }
            } else {
                panic!("Invalid Queue type");
            }
        }

        available
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct vmxnet3_softc {
    pub vmx_dev: device_t,
    pub vmx_ctx: if_ctx_t,
    pub vmx_sctx: if_shared_ctx_t,
    pub vmx_scctx: if_softc_ctx_t,
    pub vmx_ifp: *mut ifnet,
    pub vmx_ds: *mut vmxnet3_driver_shared,
    pub vmx_flags: u32,
    pub vmx_rxq: *mut RxQueue,
    pub vmx_txq: *mut TxQueue,
    pub vmx_res0: *mut resource,
    pub vmx_iot0: bus_space_tag_t,
    pub vmx_ioh0: bus_space_handle_t,
    pub vmx_res1: *mut resource,
    pub vmx_iot1: bus_space_tag_t,
    pub vmx_ioh1: bus_space_handle_t,
    pub vmx_link_active: c_int,
    pub vmx_intr_mask_mode: c_int,
    pub vmx_event_intr_idx: c_int,
    pub vmx_event_intr_irq: if_irq,
    pub vmx_mcast: *mut u8,
    pub vmx_rss: *mut vmxnet3_rss_shared,
    pub vmx_ds_dma: iflib_dma_info,
    pub vmx_qs_dma: iflib_dma_info,
    pub vmx_mcast_dma: iflib_dma_info,
    pub vmx_rss_dma: iflib_dma_info,
    pub vmx_media: *mut ifmedia,
    pub vmx_vlan_filter: [u32; 128usize],
    pub vmx_lladdr: [u8; 6usize],
}

impl Default for vmxnet3_softc {
    fn default() -> Self {
        vmxnet3_softc {
            vmx_dev: Default::default(),
            vmx_ctx: Default::default(),
            vmx_sctx: Default::default(),
            vmx_scctx: Default::default(),
            vmx_ifp: ptr::null_mut(),
            vmx_ds: ptr::null_mut(),
            vmx_flags: 0,
            vmx_rxq: ptr::null_mut(),
            vmx_txq: ptr::null_mut(),
            vmx_res0: ptr::null_mut(),
            vmx_iot0: Default::default(),
            vmx_ioh0: Default::default(),
            vmx_res1: ptr::null_mut(),
            vmx_iot1: Default::default(),
            vmx_ioh1: Default::default(),
            vmx_link_active: 0,
            vmx_intr_mask_mode: 0,
            vmx_event_intr_idx: 0,
            vmx_event_intr_irq: Default::default(),
            vmx_mcast: ptr::null_mut(),
            vmx_rss: ptr::null_mut(),
            vmx_ds_dma: Default::default(),
            vmx_qs_dma: Default::default(),
            vmx_mcast_dma: Default::default(),
            vmx_rss_dma: Default::default(),
            vmx_media: ptr::null_mut(),
            vmx_vlan_filter: [0; 128usize],
            vmx_lladdr: [0; 6usize],
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct sysctl_oid_list {
    pub _address: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct ifnet {
    pub _address: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct vmxnet3_driver_shared {
    pub _address: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct resource {
    pub _address: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct vmxnet3_rss_shared {
    pub _address: u8,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone)]
pub struct ifmedia {
    pub _address: u8,
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn txq_init() -> Result<(), VMXNet3Error> {
        let txq = TxQueue::new(0, 32, crate::pci::BarAccess::new(0, 10, 0))?;
        assert!(txq.is_empty());
        assert!(!txq.is_full());
        assert_eq!(txq.capacity(), 32);
        assert_eq!(txq.len(), 0);

        Ok(())
    }
}
