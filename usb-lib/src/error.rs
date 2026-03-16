use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsbError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid descriptor data")]
    InvalidDescriptor,

    #[error("device not found")]
    DeviceNotFound,

    #[error("permission denied — try running as administrator or check device access rules")]
    PermissionDenied,

    #[error("transfer timed out")]
    Timeout,

    #[error("endpoint stall — call reset_pipe and retry")]
    Stall,

    #[error("invalid handle — device may have been disconnected")]
    InvalidHandle,

    #[error("operation not supported on this platform")]
    Unsupported,

    #[error("USB error: {0}")]
    Other(String),
}

#[cfg(target_os = "windows")]
impl From<windows::core::Error> for UsbError {
    fn from(e: windows::core::Error) -> Self {
        match e.code().0 as u32 {
            // ERROR_ACCESS_DENIED
            0x0000_0005 => UsbError::PermissionDenied,
            // ERROR_INVALID_HANDLE
            0x0000_0006 => UsbError::InvalidHandle,
            // ERROR_FILE_NOT_FOUND
            0x0000_0002 => UsbError::DeviceNotFound,
            // ERROR_NO_SUCH_DEVICE / ERROR_DEVICE_NOT_CONNECTED
            0x0000_01B1 | 0x0000_048F => UsbError::DeviceNotFound,
            // ERROR_SEM_TIMEOUT
            0x0000_0079 => UsbError::Timeout,
            // ERROR_BAD_COMMAND (endpoint stall)
            0x0000_0016 => UsbError::Stall,
            _ => UsbError::Other(e.to_string()),
        }
    }
}
