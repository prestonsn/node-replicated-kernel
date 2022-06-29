#![allow(unused)]

use spin::Mutex;
use log::{error, info};
use arrayvec::ArrayVec;
use lazy_static::lazy_static;

use driverkit::pci::PciDevice;

/// Eventually.
/// use nvme::

use crate::environment;

/// Currenly support a maximum number of nvme devices. This should be
/// changed to something more dynamic in the future.
const MAX_NVME_DEVICES: usize = 1;

lazy_static! {
    pub(crate) static ref NVME_DEVICES: ArrayVec<Mutex<u8>, MAX_NVME_DEVICES> = {
        let mut nvme_devices = ArrayVec::new();
        for i in 1..MAX_NVME_DEVICES {
            nvme_devices.push(Mutex::new(i as u8));
        }
        nvme_devices
    };
}

#[allow(unused)]
pub(crate) fn init() {
    lazy_static::initialize(&NVME_DEVICES);
    info!("NVME: Found {} devices.", NVME_DEVICES.len());
}