use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use vmxnet3::pci::BarAccess;
use vmxnet3::smoltcp::DevQueuePhy;
use vmxnet3::vmx::VMXNet3;

use crate::memory::vspace::MapAction;
use crate::memory::PAddr;
use kpi::KERNEL_BASE;

use smoltcp::iface::{Interface, InterfaceBuilder, NeighborCache, Routes};
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address};

pub fn init_network<'a>() -> Interface<'a, DevQueuePhy> {
    const VMWARE_INC: u16 = 0x15ad;
    const VMXNET_DEV: u16 = 0x07b0;
    let pci = if let Some(vmxnet3_dev) = crate::pci::claim_device(VMWARE_INC, VMXNET_DEV) {
        let addr = vmxnet3_dev.pci_address();
        BarAccess::new(addr.bus.into(), addr.dev.into(), addr.fun.into())
    } else {
        panic!("vmxnet3 PCI device not found, forgot to pass `--nic vmxnet3`?");
    };

    // TODO(hack): Map potential vmxnet3 bar addresses XD
    // Do this in kernel space (offset of KERNEL_BASE) so the mapping persists
    let kcb = super::kcb::get_kcb();
    for &bar in &[pci.bar0 - KERNEL_BASE, pci.bar1 - KERNEL_BASE] {
        kcb.arch
            .init_vspace()
            .map_identity_with_offset(
                PAddr::from(KERNEL_BASE),
                PAddr::from(bar),
                0x1000,
                MapAction::ReadWriteKernel,
            )
            .expect("Failed to write potential vmxnet3 bar addresses")
    }

    // Create the VMX device
    let mut vmx = VMXNet3::new(pci, 1, 1).unwrap();
    vmx.attach_pre().expect("Failed to vmx.attach_pre()");
    vmx.init();

    // Create the EthernetInterface wrapping the VMX device
    let device = DevQueuePhy::new(vmx).expect("Can't create PHY");
    let neighbor_cache = NeighborCache::new(BTreeMap::new());

    // TODO: MAC, IP, and default route should be dynamic
    #[cfg(not(feature = "exokernel"))]
    let ethernet_addr = EthernetAddress([0x56, 0xb4, 0x44, 0xe9, 0x62, 0xdc]);

    #[cfg(feature = "exokernel")]
    let ethernet_addr = EthernetAddress([0x56, 0xb4, 0x44, 0xe9, 0x62, 0xdd]);

    #[cfg(not(feature = "exokernel"))]
    let ip_addrs = [IpCidr::new(IpAddress::v4(172, 31, 0, 11), 24)];

    #[cfg(feature = "exokernel")]
    let ip_addrs = [IpCidr::new(IpAddress::v4(172, 31, 0, 12), 24)];

    let mut routes = Routes::new(BTreeMap::new());
    routes
        .add_default_ipv4_route(Ipv4Address::new(172, 31, 0, 20))
        .unwrap();

    // Create SocketSet w/ space for 1 socket
    let mut sock_vec = Vec::new();
    sock_vec.try_reserve_exact(1).unwrap();
    let iface = InterfaceBuilder::new(device, sock_vec)
        .ip_addrs(ip_addrs)
        .hardware_addr(ethernet_addr.into())
        .routes(routes)
        .neighbor_cache(neighbor_cache)
        .finalize();
    iface
}
