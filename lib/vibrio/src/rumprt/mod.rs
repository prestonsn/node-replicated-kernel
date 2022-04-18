// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The rump runtime support module.
//!
//! # Organization
//!
//! File organization is as follows:
//! * The implementation to run a rumpkernel are contained in the files in the [rumprt] base.
//! * The necessary symbols for linking with `libc` are implemented in [crt] (C runtime).
//! * The necessary symbols for linking with `pthreads` are implemented in [prt] (pthread runtime).
//!
//! crt and prt could be feature gated so we could enable/disable the support at compile time,
//! however we don't do this at the moment.
//!
//! # Note on `unsafe`
//! Unfortunately, lot's of unsafe code here since pretty much everything we do has to interface
//! with the NetBSD C code, and we pass a lot of pointers etc.
//! Once the implementation grows we can think about having a safe wrapper/layer.

use crate::alloc::boxed::Box;
use crate::alloc::{alloc, format};
use core::alloc::Layout;
use core::arch::x86_64::_rdrand16_step;
use core::ffi::VaList;
use core::sync::atomic::{AtomicPtr, Ordering};
use core::{ptr, slice};

use cstr_core::CStr;

use log::{info, trace};

use lineup::mutex::Mutex;

pub mod dev;
pub mod errno;
pub mod fs;
pub mod locking;
pub mod sp;
pub mod threads;

// The crt clashes with the normal libc
#[cfg(target_os = "nrk")]
pub mod crt;
pub mod prt;

const RUMPUSER_CLOCK_RELWALL: u64 = 0;
const RUMPUSER_CLOCK_ABSMONO: u64 = 1;
const RUMPUSER_IOV_NOSEEK: i64 = -1;

#[allow(non_camel_case_types)]
pub type c_int = i32;
#[allow(non_camel_case_types)]
pub type c_uint = u32;
#[allow(non_camel_case_types)]
pub type c_long = i64;
#[allow(non_camel_case_types)]
pub type c_ulong = u64;
#[allow(non_camel_case_types)]
pub type c_void = u64;
#[allow(non_camel_case_types)]
pub type c_char = u8;
#[allow(non_camel_case_types)]
pub type c_size_t = usize;
#[allow(non_camel_case_types)]
pub type c_ssize_t = isize;

// NetBSD specific types (specific for x86_64)
#[allow(non_camel_case_types)]
pub type pid_t = u64;
#[allow(non_camel_case_types)]
pub type lwpid_t = i32;
#[allow(non_camel_case_types)]
pub type time_t = i64;
#[allow(non_camel_case_types)]
pub type clockid_t = c_uint;

/// typedef void (*rump_biodone_fn)(void *, size_t, int);
#[allow(non_camel_case_types)]
type rump_biodone_fn = Option<unsafe extern "C" fn(*mut c_void, c_size_t, c_int)>;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RumpHyperUpcalls {
    pub hyp_schedule: Option<unsafe extern "C" fn()>,
    pub hyp_unschedule: Option<unsafe extern "C" fn()>,
    pub hyp_backend_unschedule:
        Option<unsafe extern "C" fn(arg1: c_int, arg2: *mut c_int, arg3: *const c_void)>,
    pub hyp_backend_schedule: Option<unsafe extern "C" fn(arg1: c_int, arg2: *const c_void)>,
    pub hyp_lwproc_switch: Option<unsafe extern "C" fn(arg1: *mut threads::lwp)>,
    pub hyp_lwproc_release: Option<unsafe extern "C" fn()>,
    pub hyp_lwproc_rfork:
        Option<unsafe extern "C" fn(arg1: *mut c_void, arg2: c_int, arg3: *const c_char) -> c_int>,
    pub hyp_lwproc_newlwp: Option<unsafe extern "C" fn(arg1: pid_t) -> c_int>,
    pub hyp_lwproc_curlwp: Option<unsafe extern "C" fn() -> *mut threads::lwp>,
    pub hyp_syscall:
        Option<unsafe extern "C" fn(arg1: c_int, arg2: *mut c_void, arg3: *mut c_long) -> c_int>,
    pub hyp_lwpexit: Option<unsafe extern "C" fn()>,
    pub hyp_execnotify: Option<unsafe extern "C" fn(arg1: *const c_char)>,
    pub hyp_getpid: Option<unsafe extern "C" fn() -> pid_t>,
    pub hyp_extra: [*mut c_void; 8usize],
}

static HYPERUPCALLS: AtomicPtr<RumpHyperUpcalls> = AtomicPtr::new(ptr::null_mut());

#[allow(unused)]
pub fn rumpkern_curlwp() -> u64 {
    unsafe { threads::rumpuser_curlwp() as *const _ as u64 }
}

pub fn rumpkern_unsched(nlocks: &mut i32, mtx: Option<&Mutex>) {
    let upcalls = HYPERUPCALLS.load(Ordering::Relaxed) as *const RumpHyperUpcalls;

    let mtx = mtx.map_or(ptr::null(), |mtx| mtx as *const Mutex);
    unsafe {
        trace!(
            "rumpkern_unsched {} {:p} lwp={:p} upcalls = {:p}",
            nlocks,
            mtx,
            threads::rumpuser_curlwp(),
            upcalls
        );
        (*upcalls).hyp_backend_unschedule.unwrap()(0, nlocks as *mut c_int, mtx as *const u64);
    }
}

pub fn rumpkern_sched(nlocks: &i32, mtx: Option<&Mutex>) {
    let upcalls = HYPERUPCALLS.load(Ordering::Relaxed) as *const RumpHyperUpcalls;

    let mtx = mtx.map_or(ptr::null(), |mtx| mtx as *const Mutex);
    trace!("rumpkern_sched {} {:p}", *nlocks, mtx);
    unsafe {
        (*upcalls).hyp_backend_schedule.unwrap()(*nlocks, mtx as *const u64);
    }
}

// int rumpuser_init(int version, struct rump_hyperup *hyp)
#[no_mangle]
pub(crate) unsafe extern "C" fn rumpuser_init(version: i64, hyp: *mut RumpHyperUpcalls) -> i64 {
    info!("rumpuser_init ver {} {:p}", version, hyp);

    let r = HYPERUPCALLS.swap(hyp, Ordering::Relaxed);
    assert_eq!(r, ptr::null_mut(), "Can only set hyperupcalls once");

    let s = lineup::tls2::Environment::scheduler();
    s.set_rump_context(version, hyp as *mut u64);

    0
}

// int rumpuser_malloc(size_t len, int alignment, void **memp)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_malloc(
    len: usize,
    mut alignment: usize,
    memp: *mut *mut u8,
) -> i64 {
    assert!(
        len >= alignment,
        "If this doesn't hold we need a smarter deallocate method"
    );

    if alignment == 0 {
        alignment = 16;
    }

    let ptr = alloc::alloc(Layout::from_size_align_unchecked(len, alignment));
    *memp = ptr;
    0
    // ENOMEM if OOM
}

// void rumpuser_free(void *mem, size_t len)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_free(ptr: *mut u8, len: usize) {
    // We don't get the alignment on free here so we assume
    // alignment == 1, this is fine as long as rumpuser_malloc always
    // allocs with len >= alignment (see assertion there).

    trace!("rumpuser_free len={}", len);
    alloc::dealloc(ptr, Layout::from_size_align_unchecked(len, 16));
}

/// int rumpuser_getrandom(void *buf, size_t buflen, int flags, size_t *retp)
///
/// buf              buffer that the randomness is written to
/// buflen           number of bytes of randomness requested
/// flags            The value 0 or a combination of RUMPUSER_RANDOM_HARD
///                  (return true randomness instead of something from a
///                  PRNG) and RUMPUSER_RANDOM_NOWAIT (do not block in case
///                  the requested amount of bytes is not available).
/// retp             The number of random bytes written into buf.
#[no_mangle]
pub unsafe extern "C" fn rumpuser_getrandom(
    buf: *mut u8,
    buflen: usize,
    _flags: i64,
    retp: *mut usize,
) -> i64 {
    trace!("rumpuser_getrandom");

    let region: &mut [u8] = slice::from_raw_parts_mut(buf, buflen);
    for (i, ptr) in region.iter_mut().enumerate() {
        let mut rnd: u16 = 0xba;
        let ret = _rdrand16_step(&mut rnd);
        if ret == 1 {
            *ptr = rnd as u8;
        } else {
            *retp = i.checked_sub(1).unwrap_or(0);
            return 1;
        }
    }

    *retp = buflen;
    0
}

/// void rumpuser_putchar(int ch)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_putchar(ch: i64) {
    let mut buf: [u8; 4] = [0; 4]; // A buffer of length 4 is large enough to encode any char
    if ch as i64 == '\n' as u8 as i64 {
        let utf8_char = '\r'.encode_utf8(&mut buf);
        crate::syscalls::Process::print(utf8_char).expect("Can't write in rumpuser_putchar");
    }
    let utf8_char = (ch as u8 as char).encode_utf8(&mut buf);
    crate::syscalls::Process::print(utf8_char).expect("Can't write in rumpuser_putchar");
}

/// void rumpuser_dprintf(const char *fmt, ...)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_dprintf(fmt: *const i8, _ap: VaList) {
    //use core::intrinsics::VaList;
    let fmt = CStr::from_ptr(fmt).to_str().unwrap_or("");
    crate::sys_println!(" rumpuser_dprintf {}", fmt);
}

/// int rumpuser_clock_gettime(int enum_rumpclock, int64_t *sec, long *nsec)
/// enum_rumpclock   specifies the clock type.
///
/// In case of RUMPUSER_CLOCK_RELWALL the wall time should be returned.
/// In case of RUMPUSER_CLOCK_ABSMONO the time of a mono-tonic clock should be returned.
///
/// sec return value for seconds
/// nsec return value for nanoseconds
#[no_mangle]
pub unsafe extern "C" fn rumpuser_clock_gettime(
    enum_rumpclock: u64,
    sec: *mut i64,
    nsec: *mut u64,
) -> i64 {
    let boot_time = rawtime::duration_since_boot();
    trace!("rumpuser_clock_gettime {:?}", boot_time);

    match enum_rumpclock {
        RUMPUSER_CLOCK_ABSMONO => {
            *sec = boot_time.as_secs() as i64;
            *nsec = boot_time.subsec_nanos() as u64;
            0
        }
        RUMPUSER_CLOCK_RELWALL => {
            *sec = ((*rawtime::WALL_TIME_ANCHOR).as_unix_time() + boot_time.as_secs()) as i64;
            *nsec = boot_time.subsec_nanos() as u64;
            0
        }
        _ => 1,
    }
}

/// int rumpuser_getparam(const char *name, void *buf, size_t buflen)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_getparam(
    //name: *const cstr_core::c_char,
    name: *const i8,
    buf: *mut u8,
    len: usize,
) -> c_int {
    let param_name = CStr::from_ptr(name).to_str().unwrap_or("");
    trace!("rumpuser_getparam {}", param_name);

    let cstr = match param_name {
        "_RUMPUSER_NCPU" => {
            let pinfo = crate::syscalls::Process::process_info().expect("Can't read process info");
            let ncores: usize = pinfo.cmdline.parse().unwrap_or(1);
            let core_string = format!("{}\0", ncores);

            CStr::from_bytes_with_nul_unchecked(Box::leak(core_string.into_boxed_str()).as_bytes())
        }
        "RUMP_VERBOSE" => CStr::from_bytes_with_nul_unchecked(b"1\0"),
        "RUMP_THREADS" => CStr::from_bytes_with_nul_unchecked(b"1\0"),
        "_RUMPUSER_HOSTNAME" => CStr::from_bytes_with_nul_unchecked(b"btest\0"),
        "RUMP_MEMLIMIT" => CStr::from_bytes_with_nul_unchecked(b"549755813888\0"), // 512 GiB
        _ => return errno::ENOENT,
    };

    assert!(len >= cstr.to_bytes_with_nul().len());
    let buf_slice = slice::from_raw_parts_mut(buf, cstr.to_bytes_with_nul().len());
    buf_slice.copy_from_slice(cstr.to_bytes_with_nul());
    0
}

/// void rumpuser_exit(int value)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_exit(value: i64) {
    unreachable!("rumpuser_exit({})", value);
}

/// int rumpuser_kill(int64_t pid, int sig)
#[no_mangle]
pub unsafe extern "C" fn rumpuser_kill(pid: i64, sig: isize) -> isize {
    unreachable!("rumpuser_kill({}, {})", pid, sig);
}

// No need to implement:
#[no_mangle]
pub unsafe extern "C" fn rumpuser_anonmmap() {
    unreachable!("rumpuser_anonmmap");
}

#[no_mangle]
pub unsafe extern "C" fn rumpuser_unmap() {
    unreachable!("rumpuser_anonmmap");
}

#[no_mangle]
pub unsafe extern "C" fn rumpuser_daemonize_begin() {
    unreachable!("rumpuser_daemonize_begin");
}

#[no_mangle]
pub unsafe extern "C" fn rumpuser_daemonize_done() {
    unreachable!("rumpuser_daemonize_done");
}

#[no_mangle]
pub unsafe extern "C" fn rumpuser_dl_bootstrap() -> i64 {
    trace!("rumpuser_dl_bootstrap");
    0
}

#[cfg(test)]
mod test {
    use crate::rumprt::*;

    #[test]
    fn test_random() {
        unsafe {
            let mut buf: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
            let mut ret: usize = 0;
            rumpuser_getrandom(buf.as_mut_ptr(), 4, 0, &mut ret);
            assert_eq!(ret, 4);
            assert!(
                buf[0] != 0
                    && buf[1] != 0
                    && buf[2] != 0
                    && buf[3] != 0
                    && buf[4] == 0
                    && buf[5] == 0
                    && buf[6] == 0
                    && buf[7] == 0
            );
        }
    }

    #[test]
    fn test_putchar() {
        unsafe {
            rumpuser_putchar('a' as i64);
            rumpuser_putchar('b' as i64);
            rumpuser_putchar('c' as i64);
        }
    }
}
