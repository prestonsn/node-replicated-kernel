#![allow(unused)]

use spin::Mutex;
use alloc::vec::Vec;
use log::{error, info};
use lazy_static::lazy_static;

use crate::error::KResult;
use driverkit::pci::PciDevice;
use crate::pci::claim_devices_by_class_codes;

/// Eventually.
/// use nvme::

use crate::environment;


#[allow(unused)]
pub(crate) fn claim_nvme_devices<'a>() -> Vec<Option<PciDevice>> {
    /// Define PCIe baseclass and subclass code for nvme controllers.
    const NVME_BASECLASS_CODE: u8 = 1;
    const NVME_SUBCLASS_CODE: u8 = 8;
    let mut nvme_devices = claim_devices_by_class_codes(
        NVME_BASECLASS_CODE, 
        NVME_SUBCLASS_CODE
    );
    
    nvme_devices
}


#[allow(unused)]
pub(crate) fn init() {
    let mut devices: Vec<Option<PciDevice>> = claim_nvme_devices();
    
    /// DEBUG 
    for device in devices.iter() {
        if let Some(dev) = device {
            info!("Claimed NVME device: {:?}, vendor_id: {}, device_id: {}",
                dev, 
                dev.vendor_id(), 
                dev.device_id()
            );
        }
    }
}
