/// The 8-byte SETUP stage of a USB control transfer.
#[derive(Debug, Clone, Copy)]
pub struct ControlSetup {
    /// bmRequestType: direction | type | recipient
    pub request_type: u8,
    /// bRequest
    pub request: u8,
    /// wValue
    pub value: u16,
    /// wIndex
    pub index: u16,
    /// wLength — number of data bytes expected
    pub length: u16,
}

impl ControlSetup {
    /// Build a standard GET_DESCRIPTOR request.
    ///
    /// `desc_type`  — high byte of wValue (0x01=Device, 0x02=Config, 0x03=String, etc.)
    /// `desc_index` — low byte of wValue (usually 0; string index for type 0x03)
    /// `lang_id`    — wIndex (0 for non-string descriptors; 0x0409 for English strings)
    /// `length`     — number of bytes to request
    pub fn get_descriptor(desc_type: u8, desc_index: u8, lang_id: u16, length: u16) -> Self {
        Self {
            request_type: 0x80, // IN | Standard | Device
            request: 0x06,      // GET_DESCRIPTOR
            value: ((desc_type as u16) << 8) | (desc_index as u16),
            index: lang_id,
            length,
        }
    }

    /// Build a SET_CONFIGURATION request.
    pub fn set_configuration(config_value: u8) -> Self {
        Self {
            request_type: 0x00, // OUT | Standard | Device
            request: 0x09,      // SET_CONFIGURATION
            value: config_value as u16,
            index: 0,
            length: 0,
        }
    }

    /// Build a SET_INTERFACE request.
    ///
    /// `interface`   — wIndex (interface number)
    /// `alt_setting` — wValue (alternate setting to activate)
    pub fn set_interface(interface: u8, alt_setting: u8) -> Self {
        Self {
            request_type: 0x01, // OUT | Standard | Interface
            request: 0x0B,      // SET_INTERFACE
            value: alt_setting as u16,
            index: interface as u16,
            length: 0,
        }
    }

    /// Build a GET_STATUS request.
    ///
    /// `recipient` — 0 = Device, 1 = Interface, 2 = Endpoint
    /// `index`     — interface number or endpoint address (for recipient 1 and 2)
    pub fn get_status(recipient: u8, index: u16) -> Self {
        Self {
            request_type: 0x80 | (recipient & 0x1F), // IN | Standard | recipient
            request: 0x00,                           // GET_STATUS
            value: 0,
            index,
            length: 2,
        }
    }

    /// Build a CLEAR_FEATURE request.
    ///
    /// `recipient` — 0 = Device, 1 = Interface, 2 = Endpoint
    /// `feature`   — feature selector (e.g. `ENDPOINT_HALT = 0x00`)
    /// `index`     — interface number or endpoint address
    pub fn clear_feature(recipient: u8, feature: u16, index: u16) -> Self {
        Self {
            request_type: recipient & 0x1F, // OUT | Standard | recipient
            request: 0x01,                           // CLEAR_FEATURE
            value: feature,
            index,
            length: 0,
        }
    }

    /// Build a SET_FEATURE request.
    ///
    /// `recipient` — 0 = Device, 1 = Interface, 2 = Endpoint
    /// `feature`   — feature selector
    /// `index`     — interface number or endpoint address
    pub fn set_feature(recipient: u8, feature: u16, index: u16) -> Self {
        Self {
            request_type: recipient & 0x1F, // OUT | Standard | recipient
            request: 0x03,                           // SET_FEATURE
            value: feature,
            index,
            length: 0,
        }
    }
}
