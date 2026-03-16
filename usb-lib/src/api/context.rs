use crate::api::device_handle::DeviceHandle;
use crate::backend::{PlatformBackend, UsbBackend};
use crate::core::DeviceInfo;
use crate::error::UsbError;
use crate::hotplug::{HotplugEvent, HotplugHandle};

/// Top-level entry point for the USB library.
///
/// Create one `UsbContext` per application and reuse it for all operations.
pub struct UsbContext {
    backend: PlatformBackend,
}

impl UsbContext {
    /// Create a new USB context backed by the platform's native backend.
    pub fn new() -> Self {
        Self {
            backend: PlatformBackend,
        }
    }

    /// Enumerate all USB devices currently visible to the platform backend.
    pub fn devices(&self) -> Result<Vec<DeviceInfo>, UsbError> {
        self.backend.enumerate()
    }

    /// Open a device by its platform path and return a `DeviceHandle`.
    pub fn open(&self, path: &str) -> Result<DeviceHandle, UsbError> {
        let dev = self.backend.open(path)?;
        Ok(DeviceHandle::new(dev))
    }

    /// Register a callback that is invoked whenever a WinUSB device arrives or
    /// departs.  Returns a [`HotplugHandle`] that keeps the subscription alive.
    ///
    /// The callback is invoked from a **system thread** — keep it short and
    /// non-blocking.  Drop the returned handle (or call
    /// [`HotplugHandle::unregister`]) to stop receiving events.
    pub fn register_hotplug<F>(&self, callback: F) -> Result<HotplugHandle, UsbError>
    where
        F: Fn(HotplugEvent) + Send + Sync + 'static,
    {
        HotplugHandle::register(callback)
    }
}

impl Default for UsbContext {
    fn default() -> Self {
        Self::new()
    }
}
