// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! fxmark implementation for nrk.

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::num::ParseIntError;
use core::ptr;
use core::str::FromStr;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use lazy_static::lazy_static;
use log::{error, info};
use x86::bits64::paging::{PAddr, VAddr, BASE_PAGE_SIZE};

use lineup::threads::ThreadId;
use lineup::tls2::{Environment, SchedulerControlBlock};

mod drbh;
mod drbl;
mod dwol;
mod dwom;
mod mix;
mod mwrl;
mod mwrm;
use crate::fxmark::drbh::DRBH;
use crate::fxmark::drbl::DRBL;
use crate::fxmark::dwol::DWOL;
use crate::fxmark::dwom::DWOM;
use crate::fxmark::mix::MIX;
use crate::fxmark::mwrl::MWRL;
use crate::fxmark::mwrm::MWRM;

const PAGE_SIZE: u64 = 1008;

static POOR_MANS_BARRIER: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    pub static ref MAX_OPEN_FILES: AtomicUsize = { AtomicUsize::new(max_open_files()) };
}

/// This struct is used for passing the core and benchmark type from
/// the command-line/integration tests.
#[derive(Debug, PartialEq)]
pub struct ARGs {
    pub cores: usize,
    pub open_files: usize,
    pub benchmark: String,
    pub write_ratio: usize,
}

/// Both command line and integration tests pass CORExBENCH(ex: 10xdhrl). Convert
/// the string to the struct which can be used in the benchmarks.
impl FromStr for ARGs {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let coords: Vec<&str> = s.split('X').collect();

        let x_fromstr = coords[0].parse::<usize>()?;
        let open_files = coords[1].parse::<usize>()?;
        let benchmark = coords[2].to_string();
        let write_ratio = coords[3].parse::<usize>()?;
        Ok(ARGs {
            cores: x_fromstr,
            open_files,
            benchmark,
            write_ratio,
        })
    }
}

pub trait Bench {
    fn init(&self, cores: Vec<usize>, open_files: usize);
    fn run(
        &self,
        barrier: &AtomicUsize,
        duration: u64,
        core: usize,
        write_ratio: usize,
    ) -> Vec<usize>;
}

unsafe extern "C" fn fxmark_bencher_trampoline<T>(arg: *mut u8) -> *mut u8
where
    T: Bench + Default + core::marker::Send + core::marker::Sync + 'static + core::clone::Clone,
{
    let bench: Arc<MicroBench<T>> = Arc::from_raw(arg as *const MicroBench<_>);
    bench.fxmark_bencher(
        bench.cores,
        bench.benchmark,
        bench.write_ratio,
        bench.open_files,
    );
    ptr::null_mut()
}

struct MicroBench<'a, T>
where
    T: Bench + Default + core::marker::Send + core::marker::Sync + 'static + core::clone::Clone,
{
    cores: usize,
    benchmark: &'a str,
    write_ratio: usize,
    open_files: usize,
    bench: T,
}

impl<'a, T> MicroBench<'a, T>
where
    T: Bench + Default + core::marker::Send + core::marker::Sync + 'static + core::clone::Clone,
{
    pub fn new(
        cores: usize,
        benchmark: &'static str,
        write_ratio: usize,
        open_files: usize,
    ) -> MicroBench<'a, T> {
        MicroBench {
            cores,
            benchmark,
            write_ratio,
            open_files,
            bench: Default::default(),
        }
    }

    fn fxmark_bencher(&self, cores: usize, benchmark: &str, write_ratio: usize, open_files: usize) {
        let bench_duration_secs = if cfg!(feature = "smoke") { 1 } else { 10 };
        let core_id = Environment::scheduler().core_id;
        let iops = self.bench.run(
            &POOR_MANS_BARRIER,
            bench_duration_secs,
            core_id,
            write_ratio,
        );

        for iteration in 1..(bench_duration_secs + 1) {
            info!(
                "{},{},{},{},{},{},{},{}",
                core_id,
                benchmark,
                cores,
                write_ratio,
                open_files,
                bench_duration_secs,
                iteration,
                iops[iteration as usize]
            );
        }
    }
}

pub fn max_open_files() -> usize {
    let mut nodes = vibrio::syscalls::System::threads()
        .expect("Can't get system topology")
        .iter()
        .map(|c| c.node_id)
        .collect::<Vec<_>>();
    nodes.sort();
    nodes.dedup();
    nodes.len()
}

pub fn bench(ncores: Option<usize>, open_files: usize, benchmark: String, write_ratio: usize) {
    info!("thread_id,benchmark,core,write_ratio,open_files,duration_total,duration,operations");

    let hwthreads = vibrio::syscalls::System::threads().expect("Can't get system topology");
    let mut cores = Vec::with_capacity(ncores.unwrap());

    let mut maximum = 1; // We already have core 0
    for hwthread in hwthreads.iter().take(ncores.unwrap_or(hwthreads.len())) {
        cores.push(hwthread.id);
        if hwthread.id != 0 {
            match vibrio::syscalls::Process::request_core(
                hwthread.id,
                VAddr::from(vibrio::upcalls::upcall_while_enabled as *const fn() as u64),
            ) {
                Ok(_) => {
                    maximum += 1;
                    continue;
                }
                Err(e) => {
                    error!("Can't spawn on {:?}: {:?}", hwthread.id, e);
                    break;
                }
            }
        }
    }
    info!("Spawned {} cores", maximum);

    fn start<
        T: Bench + Default + core::marker::Send + core::marker::Sync + 'static + core::clone::Clone,
    >(
        maximum: usize,
        microbench: Arc<MicroBench<'static, T>>,
    ) {
        let s = &vibrio::upcalls::PROCESS_SCHEDULER;
        s.spawn(
            32 * 4096,
            move |_| {
                // use `for idx in 1..maximum+1` to run over all cores
                // currently we'll run out of 4 KiB frames
                for idx in maximum..maximum + 1 {
                    let mut thandles = Vec::with_capacity(idx);
                    // Set up barrier
                    POOR_MANS_BARRIER.store(idx, Ordering::SeqCst);

                    for core_id in 0..idx {
                        thandles.push(
                            Environment::thread()
                                .spawn_on_core(
                                    Some(fxmark_bencher_trampoline::<T>),
                                    Arc::into_raw(microbench.clone()) as *const _ as *mut u8,
                                    core_id,
                                )
                                .expect("Can't spawn bench thread?"),
                        );
                    }

                    for thandle in thandles {
                        Environment::thread().join(thandle);
                    }
                }
            },
            ptr::null_mut(),
            0,
            None,
        );

        let scb: SchedulerControlBlock = SchedulerControlBlock::new(0);
        while s.has_active_threads() {
            s.run(&scb);
        }
    }

    if benchmark == "drbl" {
        let microbench = Arc::new(MicroBench::<DRBL>::new(
            maximum,
            "drbl",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<DRBL>(maximum, microbench);
    }

    if benchmark == "drbh" {
        let microbench = Arc::new(MicroBench::<DRBH>::new(
            maximum,
            "drbh",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<DRBH>(maximum, microbench);
    }

    if benchmark == "dwol" {
        let microbench = Arc::new(MicroBench::<DWOL>::new(
            maximum,
            "dwol",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<DWOL>(maximum, microbench);
    }

    if benchmark == "dwom" {
        let microbench = Arc::new(MicroBench::<DWOM>::new(
            maximum,
            "dwom",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<DWOM>(maximum, microbench);
    }

    if benchmark == "mwrl" {
        let microbench = Arc::new(MicroBench::<MWRL>::new(
            maximum,
            "mwrl",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<MWRL>(maximum, microbench);
    }

    if benchmark == "mwrm" {
        let microbench = Arc::new(MicroBench::<MWRM>::new(
            maximum,
            "mwrm",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<MWRM>(maximum, microbench);
    }

    if benchmark == "mix" {
        let microbench = Arc::new(MicroBench::<MIX>::new(
            maximum,
            "mix",
            write_ratio,
            open_files,
        ));
        microbench.bench.init(cores.clone(), open_files);
        start::<MIX>(maximum, microbench);
    }
}
