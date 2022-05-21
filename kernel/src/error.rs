// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::convert::From;

use arrayvec::CapacityError;
use kpi::SystemCallError;

use crate::memory::VAddr;

/// Shortcut for a Result that returns an error of type KError.
pub type KResult<T> = Result<T, KError>;

/// Kernel-wide error type with everything that can potentially go wrong.
#[derive(displaydoc::Display, PartialEq, Clone, Debug)]
pub enum KError {
    /// User-space pointer is not valid
    BadAddress,
    /// Global memory is not yet available
    GlobalMemoryNotSet,
    /// The requested core is already allocated by another process
    CoreAlreadyAllocated,
    /// Ran out of memory while performing an allocation
    OutOfMemory,
    /// Replica is not set-up in the KCB
    ReplicaNotSet,
    /// The core has no current process set
    ProcessNotSet,
    /// The requested operation is not supported/does not exist
    NotSupported,
    /// Can't spawn more processes (not enough PIDs)
    OutOfPids,
    /// The core we're looking up has no executor allocated to it
    NoExecutorForCore,
    /// Invalid 1st syscall argument supplied: {a}
    InvalidSyscallArgument1 { a: u64 },
    /// Invalid VSpace Operation (2nd syscall argument) supplied: {a}
    InvalidVSpaceOperation { a: u64 },
    /// Invalid Process Operation (2nd syscall argument) supplied: {a}
    InvalidProcessOperation { a: u64 },
    /// Invalid System Operation (2nd syscall argument) supplied: {a}
    InvalidSystemOperation { a: u64 },
    /// Invalid File Operation (2nd syscall argument) supplied: {a}
    InvalidFileOperation { a: u64 },
    /// System call arguments (2) received in the wrong order
    InvalidSyscallTestArg2,
    /// System call arguments (3) received in the wrong order
    InvalidSyscallTestArg3,
    /// System call arguments (4) received in the wrong order
    InvalidSyscallTestArg4,
    /// Invalid layout for allocator provided
    InvalidLayout,
    /// Couldn't allocate bytes on this cache, need to re-grow first
    CacheExhausted,
    /// Cache can't hold any more objects
    CacheFull,
    /// Cache full -- added {count} elements
    CantGrowFurther { count: usize },
    /// KCB not set, memory allocation won't work at this point
    KcbUnavailable,
    /// The memory manager was already borrowed (this is a bug)
    ManagerAlreadyBorrowed,
    /// Specified an invalid NUMA node ID for affinity
    InvalidAffinityId,
    /// Internal data-structure grew too big
    CapacityOverflow,
    /// Can't spawn more process
    ProcessLoadingFailed,
    /// Unable to create process
    ProcessCreate,
    /// No process was associated with the given PID
    NoProcessFoundForPid,
    /// Couldn't load process (invalid ELF file?)
    UnableToLoad,
    /// Couldn't parse ELF file (invalid ELF file?)
    UnableToParseElf,
    /// We never allocated executors for this affinity region and process (need to fill cache)
    NoExecutorAllocated,
    /// The executor cache for given affinity is empty (need to refill)
    ExecutorCacheExhausted,
    /// Specified an invalid core
    InvalidGlobalThreadId,
    /// The excutor was removed from the current core
    ExecutorNoLongerValid,
    /// The executor on the core was already borrowed (that's a bug)
    ExecutorAlreadyBorrowed,
    /// Unable to reserve memory for internal process data-structures
    NotEnoughMemory,
    /// The provided FrameId is not registered with the process
    InvalidFrameId,
    /// Not enough space in process table (out of PIDs)
    TooManyProcesses,
    /// Can't register more frames with the process (out of FIDs)
    TooManyRegisteredFrames,
    /// Supplied file descriptor was invalid
    InvalidFileDescriptor,
    /// Can't spawn binary {binary}: Not found
    BinaryNotFound { binary: &'static str },
    /// Supplied frame was invalid
    InvalidFrame,
    /// Address space operation covers existing mapping {base:?}
    AlreadyMapped { base: VAddr },
    /// Provided virtual base {base:?} is invalid (led to overflow on mappings).
    BaseOverflow { base: u64 },
    /// The requested mapping is not found
    NotMapped,
    /// The supplied length is invalid
    InvalidLength,
    /// The supplied base is invalid (alignment?)
    InvalidBase,
    /// Supplied file is invalid
    InvalidFile,
    /// Supplied flags are invalid
    InvalidFlags,
    /// Supplied offset is invalid
    InvalidOffset,
    /// File/directory permission mismatch (can't be read or written)
    PermissionError,
    /// Fd or File already exists
    AlreadyPresent,
    /// Can't read or write to a directory
    DirectoryError,
    /// Can't open more files for the process
    OpenFileLimit,
    /// PID is already stored in scheduler state.
    FileDescForPidAlreadyAdded,
    /// No file-descriptors found for PID.
    NoFileDescForPid,
    /// Debugger is already attached
    DebuggerAlreadyAttached,
    /// Failure while running the GDB state machine
    DebuggerStmFailure,
    /// Can't read (debug) register
    DebuggerUnableToReadRegister,
    /// Can't write (debug) register
    DebuggerUnableToWriteRegister,
    /// Can't find a vmxnet3 device (did you pass `--nic vmxnet3`?)
    VMXNet3DeviceNotFound,
    /// Unable to initialize Ethernet device for RPC
    UnableToInitEthernetRPC,
    /// Thread-local storage was already initialized for the core
    TLSAlreadyInitialized,
}

impl From<CapacityError<crate::memory::Frame>> for KError {
    fn from(_err: CapacityError<crate::memory::Frame>) -> Self {
        KError::CacheFull
    }
}

impl From<core::cell::BorrowMutError> for KError {
    fn from(_e: core::cell::BorrowMutError) -> Self {
        KError::ManagerAlreadyBorrowed
    }
}

impl From<alloc::collections::TryReserveError> for KError {
    fn from(_e: alloc::collections::TryReserveError) -> Self {
        KError::OutOfMemory
    }
}

impl From<hashbrown::TryReserveError> for KError {
    fn from(_e: hashbrown::TryReserveError) -> Self {
        KError::OutOfMemory
    }
}

impl From<elfloader::ElfLoaderErr> for KError {
    fn from(_e: elfloader::ElfLoaderErr) -> Self {
        KError::ProcessCreate
    }
}

impl From<core::alloc::AllocError> for KError {
    fn from(_e: core::alloc::AllocError) -> Self {
        KError::OutOfMemory
    }
}

impl From<slabmalloc::AllocationError> for KError {
    fn from(err: slabmalloc::AllocationError) -> KError {
        match err {
            slabmalloc::AllocationError::InvalidLayout => KError::InvalidLayout,
            // slabmalloc OOM just means we have to refill:
            slabmalloc::AllocationError::OutOfMemory => KError::CacheExhausted,
        }
    }
}

impl From<KError> for SystemCallError {
    /// Translate KErrors to SystemCallErrors.
    ///
    /// The idea is to reduce a big set of events into a smaller set of less precise errors.
    /// We can log the the precise errors before we return in the kernel since the conversion
    /// happens at the end of the system call.
    fn from(e: KError) -> SystemCallError {
        match e {
            KError::InvalidSyscallArgument1 { .. } => SystemCallError::NotSupported,
            KError::InvalidVSpaceOperation { .. } => SystemCallError::NotSupported,
            KError::InvalidProcessOperation { .. } => SystemCallError::NotSupported,
            KError::BadAddress { .. } => SystemCallError::BadAddress,
            _ => SystemCallError::InternalError,
        }
    }
}

impl Default for KError {
    fn default() -> KError {
        KError::NotSupported
    }
}
