// Copyright © 2021 VMware, Inc. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::fxmark::{Bench, PAGE_SIZE};
use alloc::vec::Vec;
use alloc::{format, vec};
use core::cell::RefCell;
use core::slice::from_raw_parts_mut;
use core::sync::atomic::{AtomicUsize, Ordering};
use log::info;
use vibrio::io::*;

#[derive(Clone, Default)]
pub struct MWRL {}

impl Bench for MWRL {
    fn init(&self, cores: Vec<usize>, _open_files: usize) {
        unsafe {
            for core in cores {
                let fd = vibrio::syscalls::Fs::open(
                    format!("/{}/file-0.txt", core),
                    FileFlags::O_RDWR | FileFlags::O_CREAT,
                    FileModes::S_IRWXU,
                )
                .expect("FileOpen syscall failed");

                // Close the file.
                let ret = vibrio::syscalls::Fs::close(fd).expect("FileClose syscall failed");
                assert_eq!(ret, 0);
            }
        }
    }

    fn run(
        &self,
        POOR_MANS_BARRIER: &AtomicUsize,
        duration: u64,
        core: usize,
        _write_ratio: usize,
    ) -> Vec<usize> {
        use vibrio::io::*;
        use vibrio::syscalls::*;
        let mut iops_per_second = Vec::with_capacity(duration as usize);

        // Synchronize with all cores
        POOR_MANS_BARRIER.fetch_sub(1, Ordering::Release);
        while POOR_MANS_BARRIER.load(Ordering::Acquire) != 0 {
            core::sync::atomic::spin_loop_hint();
        }

        let mut iops = 0;
        let mut iterations = 0;
        let mut iter = 0;
        let filenames = vec![
            format!("/{}/file-{}.txt\0", core, 0),
            format!("/{}/file-{}.txt\0", core, 1),
        ];
        while iterations <= duration {
            let start = rawtime::Instant::now();
            while start.elapsed().as_secs() < 1 {
                for i in 0..64 {
                    let old_name = iter % 2;
                    iter += 1;
                    let new_name = iter % 2;
                    // Rename the file
                    if vibrio::syscalls::Fs::rename(
                        filenames[old_name].as_ptr() as u64,
                        filenames[new_name].as_ptr() as u64,
                    )
                    .expect("FileRename syscall failed")
                        != 0
                    {
                        panic!("FileRename syscall failed");
                    }
                    iops += 1;
                }
            }
            iops_per_second.push(iops);
            iterations += 1;
            iops = 0;
        }

        POOR_MANS_BARRIER.fetch_add(1, Ordering::Relaxed);
        iops_per_second.clone()
    }
}

unsafe impl Sync for MWRL {}
