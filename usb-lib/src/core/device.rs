/// Information about a connected USB device discovered during enumeration.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// USB Vendor ID
    pub vendor_id: u16,
    /// USB Product ID
    pub product_id: u16,
    /// USB bus number (0 if not available on this platform)
    pub bus_number: u8,
    /// USB device address on the bus (0 if not available on this platform)
    pub device_address: u8,
    /// Platform-specific device path used to open the device
    pub path: String,
    /// Manufacturer string read from the device (None if unavailable)
    pub manufacturer: Option<String>,
    /// Product string read from the device (None if unavailable)
    pub product: Option<String>,
    /// Serial number string read from the device (None if unavailable)
    pub serial_number: Option<String>,
}
