// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(warnings)]

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use cstr_core::CStr;

use fallible_collections::{FallibleVec, FallibleVecGlobal};
use klogger::{sprint, sprintln};
use log::{debug, error, info, trace, warn};
use x86::bits64::paging::{PAddr, VAddr, BASE_PAGE_SIZE, LARGE_PAGE_SIZE};
use x86::bits64::rflags;
use x86::msr::{rdmsr, wrmsr, IA32_EFER, IA32_FMASK, IA32_LSTAR, IA32_STAR};

use kpi::process::FrameId;
use kpi::{
    FileOperation, MemType, ProcessOperation, SystemCall, SystemCallError, SystemOperation,
    VSpaceOperation,
};

use crate::cmdline::{CommandLineArguments, Mode};
use crate::error::KError;
use crate::fs::{cnrfs, FileSystem};
use crate::memory::backends::PhysicalPageProvider;
use crate::memory::vspace::MapAction;
use crate::memory::{Frame, KERNEL_BASE};
use crate::process::{userptr_to_str, Pid, ResumeHandle};
use crate::syscalls::{ProcessDispatch, SystemCallDispatch, SystemDispatch, VSpaceDispatch};
use crate::{nr, nrproc};

use super::gdt::GdtTable;
use super::process::{user_virt_addr_valid, Ring3Process, UserPtr, UserValue};
use super::serial::SerialControl;

extern "C" {
    #[no_mangle]
    fn syscall_enter();
}

/// This is the core logic for handling all system calls on the x86-64
/// architecture.
///
/// The struct implements a few traits:
/// - `Arch86SystemDispatch` which in turn implements `SystemDispatch`
/// - `Arch86ProcessDispatch` which in turn implements `ProcessDispatch`
/// - `Arch86VSpaceDispatch` which in turn implements `VSpaceDispatch`
/// - `CnrDispatch` which in turn implements `FsDispatch`
///
/// The reason for having this 2-level trait system is that the rackscale
/// sub-architecture implementations currently use parts of the x86
/// implementation and this makes sharing easier (rackscale just has to opt-in
/// and out of the `Arch86*` traits to "inherit" implementation).
pub(crate) struct Arch86SystemCall;

impl SystemCallDispatch<u64> for Arch86SystemCall {}
impl Arch86SystemDispatch for Arch86SystemCall {}
impl Arch86ProcessDispatch for Arch86SystemCall {}
impl Arch86VSpaceDispatch for Arch86SystemCall {}
impl crate::syscalls::CnrFsDispatch for Arch86SystemCall {}

/// Dispatch logic for global system calls.
pub(crate) trait Arch86SystemDispatch {}

impl<T: Arch86SystemDispatch> SystemDispatch<u64> for T {
    fn get_hardware_threads(
        &self,
        vaddr_buf: u64,
        vaddr_buf_len: u64,
    ) -> Result<(u64, u64), KError> {
        // vaddr_buf = buf.as_mut_ptr() as u64
        // vaddr_buf_len = buf.len() as u64

        let hwthreads = atopology::MACHINE_TOPOLOGY.threads();
        let num_threads = atopology::MACHINE_TOPOLOGY.num_threads();

        let mut return_threads = Vec::try_with_capacity(num_threads)?;
        for hwthread in hwthreads {
            return_threads.try_push(kpi::system::CpuThread {
                id: hwthread.id as usize,
                node_id: hwthread.node_id.unwrap_or(0) as usize,
                package_id: hwthread.package_id as usize,
                core_id: hwthread.core_id as usize,
                thread_id: hwthread.thread_id as usize,
            })?;
        }

        // TODO(dependency): Get rid of serde/serde_cbor, use something sane instead
        let serialized = serde_cbor::to_vec(&return_threads).unwrap();
        if serialized.len() <= vaddr_buf_len as usize {
            let mut user_slice = super::process::UserSlice::new(vaddr_buf, serialized.len());
            user_slice.copy_from_slice(serialized.as_slice());
        }

        Ok((serialized.len() as u64, 0))
    }

    fn get_stats(&self) -> Result<(u64, u64), KError> {
        let kcb = super::kcb::get_kcb();
        info!("IRQ handler time: {} cycles", super::irq::TLB_TIME.get());
        Ok((0, 0))
    }

    fn get_core_id(&self) -> Result<(u64, u64), KError> {
        let kcb = super::kcb::get_kcb();
        Ok((
            *crate::environment::CORE_ID as u64,
            *crate::environment::NODE_ID as u64,
        ))
    }
}

/// Dispatch logic for global system calls.
pub(crate) trait Arch86ProcessDispatch {}

impl<T: Arch86ProcessDispatch> ProcessDispatch<u64> for T {
    fn log(&self, buffer_arg: u64, len: u64) -> Result<(u64, u64), KError> {
        let mut kcb = super::kcb::get_kcb();

        // TODO: some scary unsafe logic here that needs sanitization
        let buffer: *const u8 = buffer_arg as *const u8;
        let len: usize = len as usize;

        let user_str = unsafe {
            let slice = core::slice::from_raw_parts(buffer, len);
            core::str::from_utf8_unchecked(slice)
        };

        let user_buffer = UserValue::new(user_str);
        let buffer: &str = *user_buffer;
        SerialControl::buffered_print(buffer);

        Ok((0, 0))
    }

    fn get_vcpu_area(&self) -> Result<(u64, u64), KError> {
        let p = super::process::CURRENT_EXECUTOR.borrow();
        let vcpu_vaddr = p
            .as_ref()
            .ok_or(KError::NoExecutorForCore)?
            .vcpu_addr()
            .as_u64();
        Ok((vcpu_vaddr, 0))
    }

    fn allocate_vector(&self, vector: u64, core: u64) -> Result<(u64, u64), KError> {
        // TODO: missing proper IRQ resource allocation...
        super::irq::ioapic_establish_route(vector, core);
        Ok((vector, core))
    }

    fn get_process_info(&self, vaddr_buf: u64, vaddr_buf_len: u64) -> Result<(u64, u64), KError> {
        // vaddr_buf = buf.as_mut_ptr() as u64
        // vaddr_buf_len = buf.len() as u64
        let kcb = super::kcb::get_kcb();

        let pid = super::process::current_pid()?;
        let mut pinfo = nrproc::NrProcess::<Ring3Process>::pinfo(pid)?;
        pinfo.cmdline = crate::CMDLINE
            .get()
            .unwrap_or(&CommandLineArguments::default())
            .init_args;
        pinfo.app_cmdline = crate::CMDLINE
            .get()
            .unwrap_or(&CommandLineArguments::default())
            .app_args;

        let serialized = serde_cbor::to_vec(&pinfo).unwrap();
        if serialized.len() <= vaddr_buf_len as usize {
            let mut user_slice = super::process::UserSlice::new(vaddr_buf, serialized.len());
            user_slice.copy_from_slice(serialized.as_slice());
        }

        Ok((serialized.len() as u64, 0))
    }

    fn request_core(&self, core_id: u64, entry_point: u64) -> Result<(u64, u64), KError> {
        let gtid: usize = core_id.try_into().unwrap();
        let kcb = super::kcb::get_kcb();

        let mut affinity = None;
        for thread in atopology::MACHINE_TOPOLOGY.threads() {
            if thread.id == gtid {
                affinity = Some(thread.node_id.unwrap_or(0));
            }
        }
        let affinity = affinity.ok_or(KError::InvalidGlobalThreadId)?;
        let pid = super::process::current_pid()?;

        let gtid = nr::KernelNode::allocate_core_to_process(
            pid,
            VAddr::from(entry_point),
            Some(affinity),
            Some(gtid),
        )?;

        Ok((core_id, 0))
    }

    fn allocate_physical(&self, page_size: u64, affinity: u64) -> Result<(u64, u64), KError> {
        let page_size: usize = page_size.try_into().unwrap_or(0);
        //let affinity: usize = arg3.try_into().unwrap_or(0);
        // Validate input
        if page_size != BASE_PAGE_SIZE && page_size != LARGE_PAGE_SIZE {
            return Err(KError::InvalidSyscallArgument1 {
                a: page_size as u64,
            });
        }

        let pcm = super::kcb::per_core_mem();
        // Figure out what memory to allocate
        let (bp, lp) = if page_size == BASE_PAGE_SIZE {
            (1, 0)
        } else {
            (0, 1)
        };
        crate::memory::KernelAllocator::try_refill_tcache(bp, lp, MemType::Mem)?;

        // Allocate the page (need to make sure we drop pmanager again
        // before we go to NR):
        let frame = {
            let mut pmanager = pcm.mem_manager();
            if page_size == BASE_PAGE_SIZE {
                pmanager.allocate_base_page()?
            } else {
                pmanager.allocate_large_page()?
            }
        };

        // Associate memory with the process
        let pid = super::process::current_pid()?;
        let fid = nrproc::NrProcess::<Ring3Process>::allocate_frame_to_process(pid, frame)?;

        Ok((fid as u64, frame.base.as_u64()))
    }

    fn exit(&self, code: u64) -> Result<(u64, u64), KError> {
        debug!("Process got exit, we are done for now...");
        // TODO: For now just a dummy version that exits Qemu
        if code != 0 {
            // When testing we want to indicate to our integration
            // test that our user-space test failed with a non-zero exit
            super::debug::shutdown(crate::ExitReason::UserSpaceError);
        } else {
            super::debug::shutdown(crate::ExitReason::Ok);
        }
    }
}

/// Dispatch logic for vspace system calls.
pub(crate) trait Arch86VSpaceDispatch {
    fn map_generic(&self, mem_type: MemType, base: u64, size: u64) -> Result<(u64, u64), KError> {
        let base = VAddr::from(base);

        let pcm = super::kcb::per_core_mem();

        let (bp, lp) = crate::memory::utils::size_to_pages(size as usize);
        let mut frames = Vec::try_with_capacity(bp + lp)?;
        crate::memory::KernelAllocator::try_refill_tcache(20 + bp, lp, mem_type)?;

        // TODO(apihell): This `paddr` is bogus, it will return the PAddr of the
        // first frame mapped but if you map multiple Frames, no chance getting that
        // Better would be a function to request physically consecutive DMA memory
        // or use IO-MMU translation (see also rumpuser_pci_dmalloc)
        // also better to just return what NR replies with...
        let mut paddr = None;
        let mut total_len = 0;
        {
            let mut pmanager = match mem_type {
                MemType::Mem => pcm.mem_manager(),
                MemType::PMem => pcm.pmem_manager(),
                _ => unreachable!(),
            };

            for _i in 0..lp {
                let mut frame = pmanager
                    .allocate_large_page()
                    .expect("We refilled so allocation should work.");
                total_len += frame.size;
                unsafe { frame.zero() };
                frames
                    .try_push(frame)
                    .expect("Can't fail see `try_with_capacity`");
                if paddr.is_none() {
                    paddr = Some(frame.base);
                }
            }
            for _i in 0..bp {
                let mut frame = pmanager
                    .allocate_base_page()
                    .expect("We refilled so allocation should work.");
                total_len += frame.size;
                unsafe { frame.zero() };
                frames
                    .try_push(frame)
                    .expect("Can't fail see `try_with_capacity`");
                if paddr.is_none() {
                    paddr = Some(frame.base);
                }
            }
        }

        nrproc::NrProcess::<Ring3Process>::map_frames(
            crate::arch::process::current_pid()?,
            base,
            frames,
            MapAction::ReadWriteUser,
        )
        .expect("Can't map memory");

        Ok((paddr.unwrap().as_u64(), total_len as u64))
    }

    fn unmap_generic(&self, _mem_type: MemType, base: u64) -> Result<(u64, u64), KError> {
        let base = VAddr::from(base);
        let pid = super::process::current_pid()?;

        let handle = nrproc::NrProcess::<Ring3Process>::unmap(pid, base)?;
        let va: u64 = handle.vaddr.as_u64();
        let sz: u64 = handle.frame.size as u64;
        super::tlb::shootdown(handle);

        Ok((va, sz))
    }
}

impl<T: Arch86VSpaceDispatch> VSpaceDispatch<u64> for T {
    fn map_mem(&self, base: u64, size: u64) -> Result<(u64, u64), KError> {
        self.map_generic(MemType::Mem, base, size)
    }

    fn map_pmem(&self, base: u64, size: u64) -> Result<(u64, u64), KError> {
        self.map_generic(MemType::PMem, base, size)
    }

    fn map_device(&self, base: u64, size: u64) -> Result<(u64, u64), KError> {
        // TODO(safety+api): Terribly unsafe, ideally process should request/register
        // a PCI device and then it can map device things.
        let pid = super::process::current_pid()?;

        let paddr = PAddr::from(base);
        let size = size.try_into().unwrap();
        let frame = Frame::new(paddr, size, *crate::environment::NODE_ID);

        nrproc::NrProcess::<Ring3Process>::map_device_frame(pid, frame, MapAction::ReadWriteUser)
    }

    fn map_frame_id(&self, base: u64, frame_id: u64) -> Result<(u64, u64), KError> {
        let pid = super::process::current_pid()?;

        let base = VAddr::from(base);
        let frame_id: FrameId = frame_id.try_into().map_err(|_e| KError::InvalidFrameId)?;

        let (paddr, size) = nrproc::NrProcess::<Ring3Process>::map_frame_id(
            pid,
            frame_id,
            base,
            MapAction::ReadWriteUser,
        )?;
        Ok((paddr.as_u64(), size as u64))
    }

    fn unmap_mem(&self, base: u64) -> Result<(u64, u64), KError> {
        self.unmap_generic(MemType::Mem, base)
    }

    fn unmap_pmem(&self, base: u64) -> Result<(u64, u64), KError> {
        self.unmap_generic(MemType::PMem, base)
    }

    fn identify(&self, addr: u64) -> Result<(u64, u64), KError> {
        let pid = super::process::current_pid()?;
        let base = VAddr::from(addr);
        trace!("Identify address: {:#x}.", addr);
        nrproc::NrProcess::<Ring3Process>::resolve(pid, base)
    }
}

/*
#[allow(unused)]
fn debug_print_syscall(function: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64, arg5: u64) {
    sprint!("syscall: {:?}", SystemCall::new(function));

    match SystemCall::new(function) {
        SystemCall::System => {
            sprintln!(
                " {:?} {} {} {} {}",
                SystemOperation::from(arg1),
                arg2,
                arg3,
                arg4,
                arg5
            );
        }
        SystemCall::Process => {
            sprintln!(
                " {:?} {} {} {} {}",
                ProcessOperation::from(arg1),
                arg2,
                arg3,
                arg4,
                arg5
            );
        }
        SystemCall::VSpace => {
            sprintln!(
                " {:?} {} {} {} {}",
                VSpaceOperation::from(arg1),
                arg2,
                arg3,
                arg4,
                arg5
            );
        }
        SystemCall::FileIO => {
            sprintln!(
                " {:?} {} {} {} {}",
                FileOperation::from(arg1),
                arg2,
                arg3,
                arg4,
                arg5
            );
        }
        SystemCall::Test => {
            sprintln!(" {} {} {} {} {} {}", function, arg1, arg2, arg3, arg4, arg5);
        }
        SystemCall::Unknown => unreachable!(),
    }
}
 */

#[inline(never)]
#[no_mangle]
pub extern "C" fn syscall_handle(
    function: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> ! {
    //debug_print_syscall(function, arg1, arg2, arg3, arg4, arg5);
    let kcb = super::kcb::get_kcb();

    #[cfg(feature = "rackscale")]
    let status = if crate::CMDLINE
        .get()
        .map_or(false, |c| c.mode != Mode::Client)
    {
        let dispatch = Arch86SystemCall;
        dispatch.handle(function, arg1, arg2, arg3, arg4, arg5)
    } else {
        let dispatch = super::rackscale::syscalls::Arch86LwkSystemCall;
        dispatch.handle(function, arg1, arg2, arg3, arg4, arg5)
    };

    #[cfg(not(feature = "rackscale"))]
    let status = {
        let dispatch = Arch86SystemCall;
        dispatch.handle(function, arg1, arg2, arg3, arg4, arg5)
    };

    let r = {
        let _retcode = match status {
            Ok((a1, a2)) => {
                kcb.save_area.as_mut().map(|sa| {
                    sa.set_syscall_ret1(a1);
                    sa.set_syscall_ret2(a2);
                    sa.set_syscall_error_code(SystemCallError::Ok);
                });
            }
            Err(status) => {
                warn!("System call returned with error: {:?}", status);
                kcb.save_area.as_mut().map(|sa| {
                    sa.set_syscall_error_code(status.into());
                });
            }
        };

        super::process::Ring3Resumer::new_restore(kcb.get_save_area_ptr())
    };

    unsafe { r.resume() }
}

/// Enables syscall/sysret functionality.
pub(crate) fn enable_fast_syscalls() {
    let cs_selector = GdtTable::kernel_cs_selector();
    let ss_selector = GdtTable::kernel_ss_selector();

    unsafe {
        let mut star = rdmsr(IA32_STAR);
        star |= (cs_selector.bits() as u64) << 32;
        star |= (ss_selector.bits() as u64) << 48;
        wrmsr(IA32_STAR, star);

        // System call RIP
        let rip = syscall_enter as u64;
        wrmsr(IA32_LSTAR, rip);
        debug!("Set up fast syscalls. `sysenter` will jump to {:#x}.", rip);

        wrmsr(
            IA32_FMASK,
            !(rflags::RFlags::FLAGS_IOPL3 | rflags::RFlags::FLAGS_A1).bits(),
        );

        // Enable fast syscalls
        let efer = rdmsr(IA32_EFER) | 0b1;
        wrmsr(IA32_EFER, efer);
    }
}
