//! Device manager
//!
//! Adapted from MankorOS

use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};
use core::char;

use arch::interrupts::{disable_interrupt, enable_external_interrupt};
use config::{
    board,
    mm::{K_SEG_DTB_BEG, VIRT_RAM_OFFSET},
};
use device_core::{DevId, Device, DeviceMajor, DeviceMeta, DeviceType};
use log::{info, warn};
use memory::{PhysAddr, pte::PTEFlags};
use net::init_network;

use crate::{
    blk::{probe_sdio_blk, probe_vf2_sd, probe_virtio_blk},
    cpu::{CPU, probe_cpu},
    kernel_page_table_mut,
    net::{loopback::LoopbackDev, probe_virtio_net, virtio::VirtIoNetDevImpl},
    plic::{PLIC, probe_plic},
    println,
    serial::probe_char_device,
    virtio::probe_mmio_device,
};

/// The DeviceManager struct is responsible for managing the devices within the
/// system. It handles the initialization, probing, and interrupt management for
/// various devices.
pub struct DeviceManager {
    /// Optional PLIC (Platform-Level Interrupt Controller) to manage external
    /// interrupts.
    pub plic: Option<PLIC>,

    /// Vector containing CPU instances. The capacity is set to accommodate up
    /// to 8 CPUs.
    pub cpus: Vec<CPU>,

    /// A BTreeMap that maps device IDs (DevId) to device instances (Arc<dyn
    /// Device>). This map stores all the devices except for network devices
    /// which are managed separately by the `InterfaceWrapper` in the `net`
    /// module.
    pub devices: BTreeMap<DevId, Arc<dyn Device>>,

    pub net: Option<DeviceMeta>,

    /// A BTreeMap that maps interrupt numbers (irq_no) to device instances
    /// (Arc<dyn Device>). This map is used to quickly locate the device
    /// responsible for handling a specific interrupt.
    pub irq_map: BTreeMap<usize, Arc<dyn Device>>,
}

impl DeviceManager {
    /// Creates a new DeviceManager instance with default values.
    /// Initializes the PLIC to None, reserves space for 8 CPUs, and creates
    /// empty BTreeMaps for devices and irq_map.
    pub fn new() -> Self {
        Self {
            plic: None,
            cpus: Vec::with_capacity(8),
            devices: BTreeMap::new(),
            net: None,
            irq_map: BTreeMap::new(),
        }
    }

    /// mmio memory region map finished in this function
    pub fn probe(&mut self) {
        let device_tree =
            unsafe { fdt::Fdt::from_ptr(K_SEG_DTB_BEG as _).expect("Parse DTB failed") };
        if let Some(bootargs) = device_tree.chosen().bootargs() {
            println!("Bootargs: {:?}", bootargs);
        }
        println!("Device: {}", device_tree.root().model());

        if let Some(plic) = probe_plic(&device_tree) {
            self.plic = Some(plic)
        }

        if let Some(cpus) = probe_cpu(&device_tree) {
            self.cpus = cpus;
            config::board::set_harts(self.cpus.len());
        }

        if let Some(serial) = probe_char_device(&device_tree) {
            self.devices.insert(serial.dev_id(), serial);
        }

        if let Some(dev) = probe_virtio_blk(&device_tree) {
            self.devices.insert(dev.dev_id(), dev);
        }
        if let Some(dev) = probe_sdio_blk(&device_tree) {
            self.devices.insert(dev.dev_id(), dev);
        }
        // if let Some(dev) = probe_vf2_sd(&device_tree) {
        //     self.devices.insert(dev.dev_id(), dev);
        // }

        self.net = probe_virtio_net(&device_tree);

        // Add to interrupt map if have interrupts
        for dev in self.devices.values() {
            if let Some(irq) = dev.irq_no() {
                self.irq_map.insert(irq, dev.clone());
            }
        }
    }

    /// Initializes all devices that have been discovered and added to the
    /// device manager.
    pub fn init_devices(&mut self) {
        for dev in self.devices.values() {
            dev.init();
        }
    }

    pub fn init_net(&self) {
        if let Some(net_meta) = &self.net {
            let transport = probe_mmio_device(
                PhysAddr::from(net_meta.mmio_base).to_vaddr().as_mut_ptr(),
                net_meta.mmio_size,
                Some(device_core::DeviceType::Net),
            )
            .unwrap();
            let dev = VirtIoNetDevImpl::try_new(transport).unwrap();
            init_network(dev, false);
        } else {
            log::info!("[init_net] can't find qemu virtio-net. use LoopbackDev to test");
            init_network(LoopbackDev::new(), true);
        }
    }

    pub fn map_devices(&self) {
        // Map probed devices
        for (id, dev) in self.devices() {
            log::debug!("mapping device {}", dev.name());
            kernel_page_table_mut().ioremap(
                dev.mmio_base(),
                dev.mmio_size(),
                PTEFlags::R | PTEFlags::W,
            );
        }
        if let Some(net_meta) = &self.net {
            kernel_page_table_mut().ioremap(
                net_meta.mmio_base,
                net_meta.mmio_size,
                PTEFlags::R | PTEFlags::W,
            );
        }
    }

    /// Retrieves a reference to the PLIC instance. Panics if PLIC is not
    /// initialized.
    fn plic(&self) -> &PLIC {
        self.plic.as_ref().unwrap()
    }

    pub fn get(&self, dev_id: &DevId) -> Option<&Arc<dyn Device>> {
        self.devices.get(dev_id)
    }

    pub fn devices(&self) -> &BTreeMap<DevId, Arc<dyn Device>> {
        &self.devices
    }

    pub fn find_devices_by_major(&self, dmajor: DeviceMajor) -> Vec<Arc<dyn Device>> {
        self.devices()
            .iter()
            .filter(|(dev_id, _)| dev_id.major == dmajor)
            .map(|(_, dev)| dev)
            .cloned()
            .collect()
    }

    pub fn enable_device_interrupts(&mut self) {
        for i in 0..board::harts() * 2 {
            for dev in self.devices.values() {
                if let Some(irq) = dev.irq_no() {
                    self.plic().enable_irq(irq, i);
                    info!("Enable external interrupt:{irq}, context:{i}");
                }
            }
        }
        unsafe { enable_external_interrupt() }
    }

    pub fn handle_irq(&mut self) {
        unsafe { disable_interrupt() }

        log::trace!("Handling interrupt");
        // First clain interrupt from PLIC
        if let Some(irq_number) = self.plic().claim_irq(self.irq_context()) {
            if let Some(dev) = self.irq_map.get(&irq_number) {
                log::trace!(
                    "Handling interrupt from device: {:?}, irq: {}",
                    dev.name(),
                    irq_number
                );
                dev.handle_irq();
                // Complete interrupt when done
                self.plic().complete_irq(irq_number, self.irq_context());
                return;
            }
            warn!("Unknown interrupt: {}", irq_number);
        } else {
            warn!("No interrupt available");
        }
    }

    // Calculate the interrupt context from current hart id
    fn irq_context(&self) -> usize {
        // TODO:
        1
    }
}
