/// Direction of a USB endpoint transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Device-to-host (IN endpoint, bit 7 = 1)
    In,
    /// Host-to-device (OUT endpoint, bit 7 = 0)
    Out,
}

/// USB transfer type encoded in endpoint bmAttributes bits 1:0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    Control,
    Isochronous,
    Bulk,
    Interrupt,
}

/// Parsed endpoint information derived from an EndpointDescriptor.
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    /// Raw endpoint address byte (bEndpointAddress)
    pub pipe_id: u8,
    /// Endpoint number (bits 3:0 of pipe_id)
    pub number: u8,
    pub direction: Direction,
    pub transfer_type: TransferType,
    pub max_packet_size: u16,
    /// Polling interval (bInterval)
    pub interval: u8,
}

impl EndpointInfo {
    pub fn new(address: u8, attributes: u8, max_packet_size: u16, interval: u8) -> Self {
        let direction = if address & 0x80 != 0 {
            Direction::In
        } else {
            Direction::Out
        };
        let transfer_type = match attributes & 0x03 {
            0x00 => TransferType::Control,
            0x01 => TransferType::Isochronous,
            0x02 => TransferType::Bulk,
            _ => TransferType::Interrupt,
        };
        Self {
            pipe_id: address,
            number: address & 0x0F,
            direction,
            transfer_type,
            max_packet_size,
            interval,
        }
    }
}
