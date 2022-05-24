// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! KCB is the local kernel control that stores all core local state.

use alloc::sync::Arc;
use core::any::Any;

use arrayvec::ArrayVec;
use cnr::{Replica as MlnrReplica, ReplicaToken as MlnrReplicaToken};
use node_replication::{Replica, ReplicaToken};

use crate::cmdline::BootloaderArguments;
use crate::error::KError;
use crate::fs::cnrfs::MlnrKernelNode;
use crate::kcb::{ArchSpecificKcb, Kcb};
use crate::memory::mcache::TCacheSp;
use crate::nr::KernelNode;
use crate::nrproc::NrProcess;
use crate::process::{Pid, MAX_PROCESSES};

use super::process::{UnixProcess, UnixThread};
use super::{KernelArgs, MAX_NUMA_NODES};

static KERNEL_ARGS: KernelArgs = KernelArgs::new();

#[thread_local]
static mut KCB: Kcb<ArchKcb> = {
    Kcb::new(
        &[],
        BootloaderArguments::new("info", "init", "init", "init"),
        TCacheSp::new(0),
        ArchKcb::new(&KERNEL_ARGS),
        0,
    )
};

pub fn try_get_kcb<'a>() -> Option<&'a mut Kcb<ArchKcb>> {
    unsafe { Some(&mut KCB) }
}

pub fn get_kcb<'a>() -> &'a mut Kcb<ArchKcb> {
    unsafe { &mut KCB }
}

/// Initialize the KCB in the system.
///
/// Should be called during set-up. Afterwards we can use `get_kcb` safely.
pub(crate) fn init_kcb<A: ArchSpecificKcb + Any>(mut _kcb: &'static mut Kcb<A>) {
    //unreachable!("init_kcb.");
}

#[repr(C)]
pub struct ArchKcb {
    /// Arguments passed to the kernel by the bootloader.
    kernel_args: &'static KernelArgs,
    pub replica: Option<(Arc<Replica<'static, KernelNode>>, ReplicaToken)>,
    pub cnr_replica: Option<(Arc<MlnrReplica<'static, MlnrKernelNode>>, MlnrReplicaToken)>,
    pub current_executor: Option<Box<UnixThread>>,
}

impl ArchKcb {
    pub const fn new(kernel_args: &'static KernelArgs) -> ArchKcb {
        ArchKcb {
            kernel_args,
            replica: None,
            cnr_replica: None,
            current_executor: None,
        }
    }

    pub fn kernel_args(&self) -> &'static KernelArgs {
        self.kernel_args
    }

    pub fn id(&self) -> usize {
        0
    }

    pub fn max_threads(&self) -> usize {
        0
    }

    #[allow(clippy::boxed_local)]
    pub fn swap_current_executor(
        &mut self,
        _current_executor: Box<UnixThread>,
    ) -> Option<Box<UnixThread>> {
        None
    }

    pub fn has_executor(&self) -> bool {
        self.current_executor.is_some()
    }

    pub fn current_executor(&self) -> Result<&UnixThread, KError> {
        let p = self
            .current_executor
            .as_ref()
            .ok_or(KError::ProcessNotSet)?;
        Ok(p)
    }
}

impl ArchSpecificKcb for ArchKcb {
    type Process = UnixProcess;

    fn install(&mut self) {}

    fn hwthread_id(&self) -> usize {
        0
    }

    fn node(&self) -> usize {
        atopology::MACHINE_TOPOLOGY
            .current_thread()
            .node_id
            .unwrap_or(0) as usize
    }

    fn current_pid(&self) -> Result<Pid, KError> {
        Err(KError::ProcessNotSet)
    }

    #[allow(clippy::type_complexity)] // fix this once `associated_type_defaults` works
    fn process_table(
        &self,
    ) -> &'static ArrayVec<
        ArrayVec<Arc<Replica<'static, NrProcess<Self::Process>>>, MAX_PROCESSES>,
        MAX_NUMA_NODES,
    > {
        &*super::process::PROCESS_TABLE
    }
}
