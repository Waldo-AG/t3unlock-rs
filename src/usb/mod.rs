//! USB device discovery and unlock orchestration.
//!
//! Supports T1/T3/T5 Samsung Portable SSD locked variants.

mod model;
mod proto;

pub use model::Model;

use crate::errors::UsbError;
use anyhow::Context;
use rusb::{DeviceHandle, GlobalContext};
use serde::Serialize;
use std::time::Duration;

/// USB device selector
#[derive(Clone, Debug)]
pub struct DeviceSelector {
    pub vid: u16,
    pub pid: u16,
    pub model: Model,
}

impl DeviceSelector {
    pub fn new(model: Model) -> Self {
        Self {
            vid: 0x04e8,
            pid: model.locked_pid(),
            model,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Status {
    pub model: String,
    pub vid: u16,
    pub pid: u16,
    pub present: bool,
    pub locked: Option<bool>,
    pub interface: u8,
    pub ep_out: u8,
    pub ep_in: u8,
}

/// Discover device and check if it's locked.
pub fn status(sel: &DeviceSelector) -> anyhow::Result<Status> {
    let present = find_device(sel).is_ok();
    Ok(Status {
        model: sel.model.label().to_string(),
        vid: sel.vid,
        pid: sel.pid,
        present,
        locked: present.then(|| true), // assume locked until proven otherwise
        interface: proto::INTERFACE,
        ep_out: proto::EP_OUT,
        ep_in: proto::EP_IN,
    })
}

/// Perform the full unlock sequence.
pub fn unlock(sel: &DeviceSelector, password: &[u8], timeout_ms: Option<u64>) -> anyhow::Result<()> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(3000));
    let mut handle = find_device(sel).context("device not found")?;

    // Detach kernel driver if needed (Linux/macOS)
    if let Ok(true) = handle.kernel_driver_active(proto::INTERFACE) {
        handle.detach_kernel_driver(proto::INTERFACE)
            .context("failed to detach kernel driver")?;
    }

    // Claim interface
    handle
        .claim_interface(proto::INTERFACE)
        .map_err(map_libusb)?;

    // Run unlock protocol
    let result = proto::unlock(&mut handle, sel.model, password, timeout);

    // Release on error, best-effort on success
    if result.is_err() {
        let _ = handle.release_interface(proto::INTERFACE);
    } else {
        handle.release_interface(proto::INTERFACE).ok();
    }

    result
}

pub fn doctor() -> anyhow::Result<String> {
    Ok(vec![
        "=== t3unlock doctor ===",
        &format!("VID: 0x{:04x} (Samsung Electronics)", 0x04e8),
        &format!("T1 locked PID: 0x{:04x}, normal PID: 0x{:04x}", 0x61f2, 0x61f1),
        &format!("T3 locked PID: 0x{:04x}, normal PID: 0x{:04x}", 0x61f4, 0x61f3),
        &format!("T5 locked PID: 0x{:04x}, normal PID: 0x{:04x}", 0x61f6, 0x61f5),
        "",
        "Linux:",
        "  - Add udev rule: SUBSYSTEM==\"usb\", ATTR{{idVendor}}==\"04e8\", MODE=\"0666\"",
        "  - Or run as root",
        "",
        "macOS:",
        "  - No extra setup needed (root not required for bulk transfers)",
        "  - If denied, check System Preferences → Privacy & Security → USB",
        "",
        "Verify device:",
        "  Linux: lsusb -d 04e8:",
        "  macOS: system_profiler SPUSBDataType | grep -i samsung",
    ]
    .join("\n"))
}

fn find_device(sel: &DeviceSelector) -> anyhow::Result<DeviceHandle<GlobalContext>> {
    for device in rusb::devices().map_err(map_libusb)?.iter() {
        let desc = device.device_descriptor().map_err(map_libusb)?;
        if desc.vendor_id() == sel.vid && (desc.product_id() == sel.pid || desc.product_id() == sel.model.normal_pid()) {
            let handle = device.open().map_err(map_libusb)?;
            return Ok(handle);
        }
    }
    Err(anyhow::anyhow!(UsbError::NotFound))
}

fn map_libusb(e: rusb::Error) -> anyhow::Error {
    use rusb::Error::*;
    let kind = match e {
        NotFound => UsbError::NotFound,
        Access => UsbError::AccessDenied,
        Busy => UsbError::Busy,
        Timeout => UsbError::Timeout,
        Io => UsbError::Io,
        _ => UsbError::Other(e.to_string()),
    };
    anyhow::anyhow!(kind)
}
