//! # rust-usb
//!
//! A cross-platform Rust USB library with a complete Windows backend.
//!
//! ## Quick start
//! ```no_run
//! use rust_usb::UsbContext;
//!
//! let ctx = UsbContext::new();
//! for device in ctx.devices().unwrap() {
//!     println!("{:04x}:{:04x}  {}", device.vendor_id, device.product_id, device.path);
//! }
//! ```

pub mod api;
pub mod backend;
pub mod core;
pub mod error;
pub mod hotplug;

/// Tokio-backed async transfer helpers.
///
/// Import this module when the `tokio` feature is enabled:
/// ```toml
/// rust-usb = { version = "0.1", features = ["tokio"] }
/// ```
#[cfg(feature = "tokio")]
pub mod async_transfers {
    pub use crate::api::async_transfers::*;
}

// Flatten the most commonly used public types to the crate root.
pub use api::{DeviceHandle, UsbContext};
pub use core::{
    BosCapability, BosCapabilityType, BosDescriptor, ConfigDescriptor, ContainerIdCapability,
    ControlSetup, DeviceDescriptor, DeviceQualifierDescriptor, DeviceInfo, Direction,
    EndpointDescriptor, EndpointInfo, HidDescriptor, HubDescriptor, InterfaceDescriptor,
    PipePolicy, PipePolicyKind, SuperSpeedCapability, SuperSpeedEndpointCompanion,
    TransferType, Usb20ExtensionCapability,
};
pub use error::UsbError;
pub use hotplug::{HotplugEvent, HotplugHandle};
