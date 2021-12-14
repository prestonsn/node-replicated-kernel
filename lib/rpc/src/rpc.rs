// Copyright © 2021 University of Colorado. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use abomonation::Abomonation;
use alloc::vec::Vec;
use core::convert::TryInto;

#[derive(Debug, Eq, PartialEq, PartialOrd, Clone, Copy)]
pub enum RPCError {
    // RPC
    MissingData,
    ExtraData,
    TransportError,
    MalformedResponse,
    MalformedRequest,
    InternalError,
    DuplicateRPCType,

    // File IO
    InvalidFile,
    InvalidFlags,
    InvalidOffset,
    PermissionError,
    AlreadyPresent,
    DirectoryError,
    OpenFileLimit,
    FileDescForPidAlreadyAdded,
    NoFileDescForPid,

    // Syscall Errors
    InvalidSyscallArgument1 { a: u64 },
    InvalidVSpaceOperation { a: u64 },
    InvalidProcessOperation { a: u64 },
    InvalidSystemOperation { a: u64 },

    // General Errors
    BadAddress,
    NotSupported,
}
unsafe_abomonate!(RPCError);

pub type RPCType = u8;

#[derive(Debug)]
pub struct RPCHeader {
    pub client_id: u64,
    pub pid: usize,
    pub req_id: u64,
    pub msg_type: RPCType,
    pub msg_len: u64,
}
unsafe_abomonate!(RPCHeader: client_id, pid, req_id, msg_type, msg_len);

pub const HDR_LEN: usize = core::mem::size_of::<RPCHeader>();

impl Default for RPCHeader {
    fn default() -> Self {
        RPCHeader {
            client_id: 0,
            pid: 0,
            req_id: 0,
            msg_type: 0,
            msg_len: 0,
        }
    }
}

impl RPCHeader {
    /// # Safety
    /// - `self` must be valid RPCHeader
    pub unsafe fn as_mut_bytes(&mut self) -> &mut [u8; HDR_LEN] {
        ::core::slice::from_raw_parts_mut((self as *const RPCHeader) as *mut u8, HDR_LEN)
            .try_into()
            .expect("slice with incorrect length")
    }

    /// # Safety
    /// - `self` must be valid RPCHeader
    pub unsafe fn as_bytes(&self) -> &[u8; HDR_LEN] {
        ::core::slice::from_raw_parts((self as *const RPCHeader) as *const u8, HDR_LEN)
            .try_into()
            .expect("slice with incorrect length")
    }
}
