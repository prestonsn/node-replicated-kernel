#[cfg(all(feature = "integration-tests", feature = "test-time"))]
pub fn main() {
    unsafe {
        let tsc = x86::time::rdtsc();
        let tsc2 = x86::time::rdtsc();

        let start = rawtime::Instant::now();
        let done = start.elapsed().as_nanos();
        // We do this twice because I think it traps the first time?
        let start = rawtime::Instant::now();
        let done = start.elapsed().as_nanos();
        sprintln!("rdtsc overhead: {:?} cycles", tsc2 - tsc);
        sprintln!("Instant overhead: {:?} ns", done);

        if cfg!(debug_assertions) {
            assert!(tsc2 - tsc <= 100, "rdtsc overhead big?");
            // TODO: should be less:
            assert!(done <= 100, "Instant overhead big?");
        } else {
            assert!(tsc2 - tsc <= 50);
            // TODO: should be less:
            assert!(done <= 100);
        }
    }
    arch::debug::shutdown(ExitReason::Ok);
}

#[repr(C)]
struct tmpfs_args {
    ta_version: u64, // c_int
    /* Size counters. */
    ta_nodes_max: u64, // ino_t			ta_nodes_max;
    ta_size_max: i64,  // off_t			ta_size_max;
    /* Root node attributes. */
    ta_root_uid: u32,  // uid_t			ta_root_uid;
    ta_root_gid: u32,  // gid_t			ta_root_gid;
    ta_root_mode: u32, // mode_t			ta_root_mode;
}

#[cfg(all(feature = "integration-tests", feature = "test-rump-tmpfs"))]
pub fn main() {
    use cstr_core::CStr;

    extern "C" {
        fn rump_boot_setsigmodel(sig: usize);
        fn rump_init() -> u64;
        fn mount(typ: *const i8, path: *const i8, n: u64, args: *const tmpfs_args, argsize: usize);
        fn rump_component_count(fac: u64) -> u64;
        fn open(path: *const i8, opt: u64) -> i64;
        fn read(fd: i64, buf: *mut i8, bytes: u64) -> i64;
        fn write(fd: i64, buf: *const i8, bytes: u64) -> i64;
        fn rump_pub_netconfig_dhcp_ipv4_oneshot(iface: *const i8) -> i64;
    }

    let up = lineup::Upcalls {
        curlwp: rumprt::rumpkern_curlwp,
        deschedule: rumprt::rumpkern_unsched,
        schedule: rumprt::rumpkern_sched,
    };

    let mut scheduler = lineup::Scheduler::new(up);
    scheduler.spawn(
        32 * 4096,
        |_yielder| unsafe {
            let start = rawtime::Instant::now();
            rump_boot_setsigmodel(0);
            let ri = rump_init();
            assert_eq!(ri, 0);

            let TMPFS_ARGS_VERSION: u64 = 1;

            let tfsa = tmpfs_args {
                ta_version: TMPFS_ARGS_VERSION,
                ta_nodes_max: 0,
                ta_size_max: 1 * 1024 * 1024,
                ta_root_uid: 0,
                ta_root_gid: 0,
                ta_root_mode: 0o1777,
            };

            let path = CStr::from_bytes_with_nul(b"/tmp\0");
            let MOUNT_TMPFS = CStr::from_bytes_with_nul(b"tmpfs\0");
            info!("mounting tmpfs");
            let r = mount(
                MOUNT_TMPFS.unwrap().as_ptr(),
                path.unwrap().as_ptr(),
                0,
                &tfsa,
                core::mem::size_of::<tmpfs_args>(),
            );
            info!("rump___sysimpl_mount50: {:?}", r);

            //assert_eq!(r, 0, "Successfully mounted tmpfs");

            let path = CStr::from_bytes_with_nul(b"/tmp/bla\0");
            let fd = open(path.unwrap().as_ptr(), 0x00000202);
            assert_eq!(fd, 3, "Proper FD was returned");

            let wbuf: [i8; 12] = [0xa; 12];
            let bytes_written = write(fd, wbuf.as_ptr(), 12);
            assert_eq!(bytes_written, 12, "Write successful");
            info!("bytes_written: {:?}", bytes_written);

            let path = CStr::from_bytes_with_nul(b"/tmp/bla\0");
            let fd = open(path.unwrap().as_ptr(), 0x00000002);
            let mut rbuf: [i8; 12] = [0x00; 12];
            let read_bytes = read(fd, rbuf.as_mut_ptr(), 12);
            assert_eq!(read_bytes, 12, "Read successful");
            assert_eq!(rbuf[0], 0xa, "Read matches write");

            info!(
                "rump_init({}) done in {:?}, mounted tmpfs",
                ri,
                start.elapsed()
            );
        },
        core::ptr::null_mut(),
    );

    for i in 0..9999 {
        scheduler.run();
    }

    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-rump"))]
pub fn main() {
    use cstr_core::CStr;

    extern "C" {
        fn rump_boot_setsigmodel(sig: usize);
        fn rump_init() -> u64;
        fn rump_pub_netconfig_ifcreate(iface: *const i8) -> i64;
        fn rump_pub_netconfig_dhcp_ipv4_oneshot(iface: *const i8) -> i64;
        fn rump_pub_netconfig_ipv4_gw(addr: *const i8) -> i64;
        fn rump_pub_netconfig_ipv4_ifaddr_cidr(iface: *const i8, addr: *const i8, mask: u32)
            -> i64;
        fn mount(typ: *const i8, path: *const i8, n: u64, args: *const tmpfs_args, argsize: usize);

        fn sysctlbyname(
            name: *const i8,
            oldp: *mut rumprt::c_void,
            oldlenp: *mut rumprt::c_size_t,
            newp: *const rumprt::c_void,
            newlen: rumprt::c_size_t,
        ) -> rumprt::c_int;
    }

    let up = lineup::Upcalls {
        curlwp: rumprt::rumpkern_curlwp,
        deschedule: rumprt::rumpkern_unsched,
        schedule: rumprt::rumpkern_sched,
    };

    let mut scheduler = lineup::Scheduler::new(up);
    scheduler.spawn(
        32 * 4096,
        |_yielder| unsafe {
            let start = rawtime::Instant::now();
            rump_boot_setsigmodel(0);
            let ri = rump_init();
            assert_eq!(ri, 0);
            let TMPFS_ARGS_VERSION: u64 = 1;

            let tfsa = tmpfs_args {
                ta_version: TMPFS_ARGS_VERSION,
                ta_nodes_max: 0,
                ta_size_max: 1 * 1024 * 1024,
                ta_root_uid: 0,
                ta_root_gid: 0,
                ta_root_mode: 0o1777,
            };

            let path = CStr::from_bytes_with_nul(b"/tmp\0");
            let MOUNT_TMPFS = CStr::from_bytes_with_nul(b"tmpfs\0");
            info!("mounting tmpfs");
            let r = mount(
                MOUNT_TMPFS.unwrap().as_ptr(),
                path.unwrap().as_ptr(),
                0,
                &tfsa,
                core::mem::size_of::<tmpfs_args>(),
            );
            info!("rump___sysimpl_mount50: {:?}", r);

            //const char network_format_string[] = "\"net\" :  {,,\"if\":\"wm0\",, \"type\":\"inet\",,\"method\":\"dhcp\",,},,";

            /*let path = CStr::from_bytes_with_nul(b"wm0\0");
            let r = rump_pub_netconfig_ifcreate(path.unwrap().as_ptr());
            assert_eq!(r, 0, "rump_pub_netconfig_ifcreate");*/

            /*use core::mem;
            use core::ptr;
            let dad_count = CStr::from_bytes_with_nul(b"net.inet.ip.dad_count\0");
            let x: rumprt::c_size_t = 0;
            sysctlbyname(
                dad_count.unwrap().as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                &x as *const rumprt::c_size_t as *const rumprt::c_void,
                mem::size_of::<rumprt::c_size_t>() as rumprt::c_size_t,
            );*/

            let path = CStr::from_bytes_with_nul(b"wm0\0");
            let r = rump_pub_netconfig_dhcp_ipv4_oneshot(path.unwrap().as_ptr());
            assert_eq!(r, 0, "rump_pub_netconfig_dhcp_ipv4_oneshot");

            /*let path = CStr::from_bytes_with_nul(b"wm0\0");
            let addr = CStr::from_bytes_with_nul(b"10.0.120.101/24\0");
            let r = rump_pub_netconfig_ipv4_ifaddr_cidr(
                path.unwrap().as_ptr(),
                addr.unwrap().as_ptr(),
                0,
            );
            assert_eq!(r, 0, "rump_pub_netconfig_ipv4_ifaddr_cidr");
            unreachable!();*/

            /*let gw = CStr::from_bytes_with_nul(b"10.0.120.100/24\0");
            let r = rump_pub_netconfig_ipv4_gw(gw.unwrap().as_ptr());
            assert_eq!(r, 0, "rump_pub_netconfig_ipv4_gw");*/

            info!(
                "rump_init({}) done in {:?}, mounted tmpfs",
                ri,
                start.elapsed()
            );
            loop {
                lineup::tls::Environment::thread().sleep(rawtime::Duration::from_secs(1));
            }
        },
        core::ptr::null_mut(),
    );

    scheduler
        .spawn(
            32 * 1024,
            |_yielder| unsafe {
                rumprt::dev::irq_handler(core::ptr::null_mut());
            },
            core::ptr::null_mut(),
        )
        .expect("Can't create IRQ thread?");

    //for _i in 0..99999 {
    loop {
        scheduler.run();
    }

    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-buddy"))]
pub fn main() {
    use buddy::FreeBlock;
    use buddy::Heap;
    let mut heap = Heap::new(
        heap_base: *mut u8,
        heap_size: usize,
        free_lists: &mut [*mut FreeBlock],
    );

    let b = heap.allocate(4096, 4096);

    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-exit"))]
pub fn main() {
    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-pfault"))]
pub fn main() {
    use arch::memory::{paddr_to_kernel_vaddr, PAddr};

    unsafe {
        let paddr = PAddr::from(1024 * 1024 * 1024 * 1);
        let kernel_vaddr = paddr_to_kernel_vaddr(paddr);
        let ptr: *mut u64 = kernel_vaddr.as_mut_ptr();
        let val = *ptr;
        assert!(val != 0);
    }
}

#[cfg(all(feature = "integration-tests", feature = "test-gpfault"))]
pub fn main() {
    // Note that int!(13) doesn't work in qemu. It doesn't push an error code properly for it.
    // So we cause a GP by loading garbage in the ss segment register.
    use x86::segmentation::{load_ss, SegmentSelector};
    unsafe {
        load_ss(SegmentSelector::new(99, x86::Ring::Ring3));
    }
}

#[cfg(all(feature = "integration-tests", feature = "test-alloc"))]
pub fn main() {
    use alloc::vec::Vec;
    {
        let mut buf: Vec<u8> = Vec::with_capacity(0);
        for i in 0..1024 {
            buf.push(i as u8);
        }
    } // Make sure we drop here.
    debug!("small allocations work.");

    {
        let size: usize = x86::bits64::paging::BASE_PAGE_SIZE;
        let mut buf: Vec<u8> = Vec::with_capacity(size);
        for i in 0..size {
            buf.push(i as u8);
        }

        let size: usize = x86::bits64::paging::BASE_PAGE_SIZE * 256;
        let mut buf: Vec<usize> = Vec::with_capacity(size);
        for i in 0..size {
            buf.push(i as usize);
        }
    } // Make sure we drop here.
    debug!("large allocations work.");
    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-scheduler"))]
pub fn main() {
    let cpuid = x86::cpuid::CpuId::new();
    assert!(
        cpuid
            .get_extended_feature_info()
            .map_or(false, |ef| ef.has_fsgsbase()),
        "FS/GS base instructions supported"
    );
    use lineup::tls::Environment;

    let mut s = lineup::Scheduler::new(lineup::DEFAULT_UPCALLS);
    s.spawn(
        4096,
        |arg| {
            let _r = Environment::thread().relinquish();
            debug!("lwt1 {:?}", Environment::tid());
        },
        core::ptr::null_mut(),
    );

    s.spawn(
        4096,
        |arg| {
            debug!("lwt2 {:?}", Environment::tid());
        },
        core::ptr::null_mut(),
    );

    s.run();
    s.run();
    s.run();
    s.run();

    arch::debug::shutdown(ExitReason::Ok);
}

#[cfg(all(feature = "integration-tests", feature = "test-sse"))]
pub fn main() {
    info!("division = {}", 10.0 / 2.19);
    info!("division by zero = {}", 10.0 / 0.0);
    arch::debug::shutdown(ExitReason::Ok);
}
