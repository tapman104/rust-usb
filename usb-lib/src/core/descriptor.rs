use crate::error::UsbError;

/// Parsed USB Device Descriptor (18 bytes, bDescriptorType = 0x01).
#[derive(Debug, Clone)]
pub struct DeviceDescriptor {
    pub bcd_usb: u16,
    pub device_class: u8,
    pub device_sub_class: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bcd_device: u16,
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_number_index: u8,
    pub num_configurations: u8,
}

impl DeviceDescriptor {
    /// Parse from a raw 18-byte buffer returned by GET_DESCRIPTOR(Device).
    pub fn from_bytes(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 18 {
            return Err(UsbError::InvalidDescriptor);
        }
        if buf[1] != 0x01 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            bcd_usb: u16::from_le_bytes([buf[2], buf[3]]),
            device_class: buf[4],
            device_sub_class: buf[5],
            device_protocol: buf[6],
            max_packet_size0: buf[7],
            vendor_id: u16::from_le_bytes([buf[8], buf[9]]),
            product_id: u16::from_le_bytes([buf[10], buf[11]]),
            bcd_device: u16::from_le_bytes([buf[12], buf[13]]),
            manufacturer_index: buf[14],
            product_index: buf[15],
            serial_number_index: buf[16],
            num_configurations: buf[17],
        })
    }
}

/// Parsed USB Configuration Descriptor (9-byte header + nested descriptors).
#[derive(Debug, Clone)]
pub struct ConfigDescriptor {
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration_index: u8,
    pub attributes: u8,
    pub max_power: u8,
    pub interfaces: Vec<InterfaceDescriptor>,
}

impl ConfigDescriptor {
    /// Parse a full configuration descriptor blob (first 9 bytes are the header;
    /// the remaining bytes contain Interface and Endpoint descriptors).
    pub fn from_bytes(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 9 {
            return Err(UsbError::InvalidDescriptor);
        }
        if buf[1] != 0x02 {
            return Err(UsbError::InvalidDescriptor);
        }
        let total_length = u16::from_le_bytes([buf[2], buf[3]]);
        let num_interfaces = buf[4];
        let configuration_value = buf[5];
        let configuration_index = buf[6];
        let attributes = buf[7];
        let max_power = buf[8];

        // Walk the descriptor chain after the 9-byte config header.
        let mut interfaces: Vec<InterfaceDescriptor> = Vec::new();
        let mut pos = 9usize;
        let end = (total_length as usize).min(buf.len());

        while pos + 2 <= end {
            let b_length = buf[pos] as usize;
            let b_type = buf[pos + 1];

            if b_length < 2 || pos + b_length > end {
                break;
            }

            if b_type == 0x04 {
                // Interface descriptor
                if b_length >= 9 {
                    interfaces.push(InterfaceDescriptor::from_bytes_at(&buf[pos..pos + b_length])?);
                }
            } else if b_type == 0x05 {
                // Endpoint descriptor — attach to last interface
                if b_length >= 7 {
                    let ep = EndpointDescriptor::from_bytes_at(&buf[pos..pos + b_length])?;
                    if let Some(iface) = interfaces.last_mut() {
                        iface.endpoints.push(ep);
                    }
                }
            } else if b_type == 0x21 {
                // HID descriptor — attach to last interface
                if b_length >= 9 {
                    let hid = HidDescriptor::from_bytes_at(&buf[pos..pos + b_length])?;
                    if let Some(iface) = interfaces.last_mut() {
                        iface.hid_descriptor = Some(hid);
                    }
                }
            } else if b_type == 0x30 {
                // SuperSpeed Endpoint Companion — attach to the last endpoint of the last interface
                if b_length >= 6 {
                    if let Ok(ssc) = SuperSpeedEndpointCompanion::from_bytes_at(&buf[pos..pos + b_length]) {
                        if let Some(iface) = interfaces.last_mut() {
                            if let Some(ep) = iface.endpoints.last_mut() {
                                ep.ss_companion = Some(ssc);
                            }
                        }
                    }
                }
            }
            // Other descriptor types (IAD 0x0B, etc.) are skipped.

            pos += b_length;
        }

        Ok(Self {
            total_length,
            num_interfaces,
            configuration_value,
            configuration_index,
            attributes,
            max_power,
            interfaces,
        })
    }
}

/// Parsed USB Interface Descriptor (9 bytes, bDescriptorType = 0x04).
#[derive(Debug, Clone)]
pub struct InterfaceDescriptor {
    pub interface_number: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_sub_class: u8,
    pub interface_protocol: u8,
    pub interface_index: u8,
    pub endpoints: Vec<EndpointDescriptor>,
    /// HID class descriptor, present when `interface_class == 0x03`.
    pub hid_descriptor: Option<HidDescriptor>,
}

impl InterfaceDescriptor {
    fn from_bytes_at(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 9 || buf[1] != 0x04 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            interface_number: buf[2],
            alternate_setting: buf[3],
            num_endpoints: buf[4],
            interface_class: buf[5],
            interface_sub_class: buf[6],
            interface_protocol: buf[7],
            interface_index: buf[8],
            endpoints: Vec::new(),
            hid_descriptor: None,
        })
    }
}

/// Parsed USB Endpoint Descriptor (7 bytes, bDescriptorType = 0x05).
#[derive(Debug, Clone)]
pub struct EndpointDescriptor {
    pub endpoint_address: u8,
    pub attributes: u8,
    pub max_packet_size: u16,
    pub interval: u8,
    /// Present only for SuperSpeed endpoints (bDescriptorType = 0x30, immediately follows).
    pub ss_companion: Option<SuperSpeedEndpointCompanion>,
}

impl EndpointDescriptor {
    fn from_bytes_at(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 7 || buf[1] != 0x05 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            endpoint_address: buf[2],
            attributes: buf[3],
            max_packet_size: u16::from_le_bytes([buf[4], buf[5]]),
            interval: buf[6],
            ss_companion: None,
        })
    }
}

// ---------------------------------------------------------------------------
// SuperSpeed Endpoint Companion Descriptor (6 bytes, bDescriptorType = 0x30)
// Immediately follows an Endpoint descriptor in SuperSpeed configurations.
// ---------------------------------------------------------------------------

/// SuperSpeed Endpoint Companion Descriptor (USB 3.x).
#[derive(Debug, Clone)]
pub struct SuperSpeedEndpointCompanion {
    /// Maximum number of additional packets per service interval (0–15 for bulk/interrupt).
    pub max_burst: u8,
    /// Endpoint type-specific attributes (streams for bulk, mult for isochronous).
    pub attributes: u8,
    /// Total bytes per service interval (isochronous/interrupt only; 0 for bulk).
    pub bytes_per_interval: u16,
}

impl SuperSpeedEndpointCompanion {
    pub(crate) fn from_bytes_at(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 6 || buf[1] != 0x30 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            max_burst: buf[2],
            attributes: buf[3],
            bytes_per_interval: u16::from_le_bytes([buf[4], buf[5]]),
        })
    }
}

// ---------------------------------------------------------------------------
// Device Qualifier Descriptor (10 bytes, bDescriptorType = 0x06)
// Returned by high-speed capable devices when operating at full speed (and v.v.)
// ---------------------------------------------------------------------------

/// Device Qualifier Descriptor — describes full/high-speed characteristics
/// of a device that supports both speeds.
#[derive(Debug, Clone)]
pub struct DeviceQualifierDescriptor {
    pub bcd_usb: u16,
    pub device_class: u8,
    pub device_sub_class: u8,
    pub device_protocol: u8,
    pub max_packet_size0: u8,
    pub num_configurations: u8,
}

impl DeviceQualifierDescriptor {
    /// Parse from a raw 10-byte GET_DESCRIPTOR(Device_Qualifier) response.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 10 {
            return Err(UsbError::InvalidDescriptor);
        }
        if buf[1] != 0x06 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            bcd_usb: u16::from_le_bytes([buf[2], buf[3]]),
            device_class: buf[4],
            device_sub_class: buf[5],
            device_protocol: buf[6],
            max_packet_size0: buf[7],
            num_configurations: buf[8],
        })
    }
}

// ---------------------------------------------------------------------------
// HID Descriptor (9 bytes minimum, bDescriptorType = 0x21)
// Embedded inside a Configuration descriptor after the Interface descriptor.
// ---------------------------------------------------------------------------

/// HID class descriptor embedded within the configuration descriptor blob.
#[derive(Debug, Clone)]
pub struct HidDescriptor {
    pub bcd_hid: u16,
    pub country_code: u8,
    /// Number of subordinate class descriptors.
    pub num_descriptors: u8,
    /// Type of the first subordinate descriptor (0x22 = Report).
    pub descriptor_type: u8,
    /// Length of the first subordinate descriptor in bytes.
    pub descriptor_length: u16,
}

impl HidDescriptor {
    fn from_bytes_at(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 9 || buf[1] != 0x21 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(Self {
            bcd_hid: u16::from_le_bytes([buf[2], buf[3]]),
            country_code: buf[4],
            num_descriptors: buf[5],
            descriptor_type: buf[6],
            descriptor_length: u16::from_le_bytes([buf[7], buf[8]]),
        })
    }
}

// ---------------------------------------------------------------------------
// BOS Descriptor (Binary Object Store, bDescriptorType = 0x0F)
// Introduced in USB 2.0 ECN, required for USB 3.x.
// ---------------------------------------------------------------------------

/// Raw capability type constants embedded inside a BOS descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BosCapabilityType {
    WirelessUsb = 0x01,
    Usb20Extension = 0x02,
    SuperSpeedUsb = 0x03,
    ContainerId = 0x04,
    Platform = 0x05,
    SuperSpeedPlus = 0x0A,
    Unknown(u8),
}

impl From<u8> for BosCapabilityType {
    fn from(v: u8) -> Self {
        match v {
            0x01 => Self::WirelessUsb,
            0x02 => Self::Usb20Extension,
            0x03 => Self::SuperSpeedUsb,
            0x04 => Self::ContainerId,
            0x05 => Self::Platform,
            0x0A => Self::SuperSpeedPlus,
            other => Self::Unknown(other),
        }
    }
}

/// USB 2.0 Extension capability (bDevCapabilityType = 0x02, 7 bytes total).
#[derive(Debug, Clone)]
pub struct Usb20ExtensionCapability {
    /// bmAttributes — bit 1: LPM support, bit 2: BESL support.
    pub attributes: u32,
}

/// SuperSpeed USB device capability (bDevCapabilityType = 0x03, 10 bytes total).
#[derive(Debug, Clone)]
pub struct SuperSpeedCapability {
    pub attributes: u8,
    /// Bitmap of supported speeds: bit 1=FS, bit 2=HS, bit 3=SS.
    pub speed_supported: u16,
    /// Lowest speed at which all functionality is available.
    pub functionality_support: u8,
    /// U1 device exit latency (μs).
    pub u1_dev_exit_lat: u8,
    /// U2 device exit latency (μs × 256).
    pub u2_dev_exit_lat: u16,
}

/// Container ID capability (bDevCapabilityType = 0x04, 20 bytes total).
#[derive(Debug, Clone)]
pub struct ContainerIdCapability {
    pub container_id: [u8; 16],
}

/// A single capability record parsed from a BOS descriptor.
#[derive(Debug, Clone)]
pub enum BosCapability {
    Usb20Extension(Usb20ExtensionCapability),
    SuperSpeedUsb(SuperSpeedCapability),
    ContainerId(ContainerIdCapability),
    /// Any capability not explicitly understood; carries the raw bytes.
    Unknown { cap_type: u8, data: Vec<u8> },
}

/// Binary Object Store (BOS) descriptor, returned by GET_DESCRIPTOR(BOS, 0).
#[derive(Debug, Clone)]
pub struct BosDescriptor {
    pub total_length: u16,
    pub num_device_caps: u8,
    pub capabilities: Vec<BosCapability>,
}

impl BosDescriptor {
    /// Parse a full BOS descriptor blob (must contain the complete wTotalLength bytes).
    pub fn from_bytes(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 5 {
            return Err(UsbError::InvalidDescriptor);
        }
        if buf[1] != 0x0F {
            return Err(UsbError::InvalidDescriptor);
        }
        let total_length = u16::from_le_bytes([buf[2], buf[3]]);
        let num_caps = buf[4];
        let end = (total_length as usize).min(buf.len());

        let mut capabilities = Vec::new();
        let mut pos = 5usize; // skip the 5-byte BOS header

        while pos + 3 <= end {
            let b_length = buf[pos] as usize;
            let b_desc_type = buf[pos + 1];
            let b_cap_type = buf[pos + 2];

            if b_length < 3 || pos + b_length > end {
                break;
            }
            if b_desc_type != 0x10 {
                // Not a Device Capability descriptor — skip
                pos += b_length;
                continue;
            }

            let cap_data = &buf[pos..pos + b_length];
            let cap = match BosCapabilityType::from(b_cap_type) {
                BosCapabilityType::Usb20Extension if b_length >= 7 => {
                    BosCapability::Usb20Extension(Usb20ExtensionCapability {
                        attributes: u32::from_le_bytes([cap_data[3], cap_data[4], cap_data[5], cap_data[6]]),
                    })
                }
                BosCapabilityType::SuperSpeedUsb if b_length >= 10 => {
                    BosCapability::SuperSpeedUsb(SuperSpeedCapability {
                        attributes: cap_data[3],
                        speed_supported: u16::from_le_bytes([cap_data[4], cap_data[5]]),
                        functionality_support: cap_data[6],
                        u1_dev_exit_lat: cap_data[7],
                        u2_dev_exit_lat: u16::from_le_bytes([cap_data[8], cap_data[9]]),
                    })
                }
                BosCapabilityType::ContainerId if b_length >= 20 => {
                    let mut id = [0u8; 16];
                    id.copy_from_slice(&cap_data[4..20]);
                    BosCapability::ContainerId(ContainerIdCapability { container_id: id })
                }
                _ => BosCapability::Unknown {
                    cap_type: b_cap_type,
                    data: cap_data[3..].to_vec(),
                },
            };
            capabilities.push(cap);
            pos += b_length;
        }

        Ok(Self { total_length, num_device_caps: num_caps, capabilities })
    }
}

// ---------------------------------------------------------------------------
// Hub Descriptor (bDescriptorType = 0x29)
// Returned by a USB hub via a class-specific GET_DESCRIPTOR request.
// ---------------------------------------------------------------------------

/// USB Hub Descriptor (bDescriptorType = 0x29).
#[derive(Debug, Clone)]
pub struct HubDescriptor {
    /// Number of downstream-facing ports.
    pub num_ports: u8,
    /// wHubCharacteristics — logical power switching, compound device, over-current protection, etc.
    pub hub_characteristics: u16,
    /// Time (in 2 ms units) from power-on to when the port is ready (bPwrOn2PwrGood).
    pub power_on_to_power_good: u8,
    /// Maximum current requirements of the hub controller (mA).
    pub hub_controller_current: u8,
    /// Per-port removable device bitmask (one bit per port starting at bit 1).
    pub device_removable: u16,
}

impl HubDescriptor {
    /// Parse from the raw buffer returned by the hub GET_DESCRIPTOR(Hub) request.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, UsbError> {
        if buf.len() < 9 {
            return Err(UsbError::InvalidDescriptor);
        }
        if buf[1] != 0x29 {
            return Err(UsbError::InvalidDescriptor);
        }
        let num_ports = buf[2];
        let hub_characteristics = u16::from_le_bytes([buf[3], buf[4]]);
        let power_on_to_power_good = buf[5];
        let hub_controller_current = buf[6];
        // DeviceRemovable is a variable-length bitmask starting at byte 7.
        // We read up to 2 bytes (supports up to 15 ports).
        let removable_lo = buf[7];
        let removable_hi = if buf.len() > 8 { buf[8] } else { 0 };
        let device_removable = u16::from_le_bytes([removable_lo, removable_hi]);

        Ok(Self {
            num_ports,
            hub_characteristics,
            power_on_to_power_good,
            hub_controller_current,
            device_removable,
        })
    }
}

