use std::fs::{File, OpenOptions};
use std::os::unix::io::AsRawFd;
use std::sync::Mutex;
use std::time::Duration;

use nix::libc;
use nix::{request_code_none, request_code_read, request_code_readwrite, request_code_write};

use crate::core::{
    ConfigDescriptor, ControlSetup, DeviceDescriptor, DeviceInfo, EndpointInfo, PipePolicy,
    PipePolicyKind,
};
use crate::error::UsbError;

use super::{UsbBackend, UsbDevice};

// -----------------------------------------------------------------------
// Linux kernel USBDEVFS ioctl constants (from linux/usbdevice_fs.h)
// -----------------------------------------------------------------------

const USBDEVFS_CONTROL_IOCTL: u8 = b'U';
const USBDEVFS_CONTROL_NR: u8 = 0;
const USBDEVFS_BULK_NR: u8 = 2;
const USBDEVFS_RESETEP_NR: u8 = 3;
const USBDEVFS_SETINTERFACE_NR: u8 = 4;
const USBDEVFS_CLAIMINTERFACE_NR: u8 = 15;
const USBDEVFS_RELEASEINTERFACE_NR: u8 = 16;
const USBDEVFS_RESET_NR: u8 = 20;
const USBDEVFS_CLEAR_HALT_NR: u8 = 21;

#[cfg(feature = "isochronous")]
const USBDEVFS_SUBMITURB_NR: u8 = 10;
#[cfg(feature = "isochronous")]
const USBDEVFS_REAPURB_NR: u8 = 12;
#[cfg(feature = "isochronous")]
const USBDEVFS_URB_TYPE_ISO: u8 = 0;
#[cfg(feature = "isochronous")]
const MAX_ISO_PACKETS: usize = 256;

/// Matches `struct usbdevfs_ctrltransfer` from linux/usbdevice_fs.h
#[repr(C)]
struct UsbdevfsCtrltransfer {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
    timeout: u32, // milliseconds
    data: *mut libc::c_void,
}

/// Matches `struct usbdevfs_bulktransfer` from linux/usbdevice_fs.h
#[repr(C)]
struct UsbdevfsBulktransfer {
    /// Endpoint address (e.g. 0x81 for EP1-IN).
    ep: u32,
    /// Number of bytes to transfer.
    len: u32,
    /// Timeout in milliseconds.
    timeout: u32,
    /// Data buffer pointer.
    data: *mut libc::c_void,
}

/// Matches `struct usbdevfs_setinterface` from linux/usbdevice_fs.h
#[repr(C)]
struct UsbdevfsSetinterface {
    interface: u32,
    altsetting: u32,
}

#[cfg(feature = "isochronous")]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct UsbdevfsIsoPacketDesc {
    length: u32,
    actual_length: u32,
    status: u32,
}

/// Header portion of `struct usbdevfs_urb` (before the flexible `iso_frame_desc[]` tail).
#[cfg(feature = "isochronous")]
#[repr(C)]
#[derive(Default)]
struct UsbdevfsUrbHeader {
    urb_type: u8,
    endpoint: u8,
    status: i32,
    flags: u32,
    buffer: *mut libc::c_void,
    buffer_length: i32,
    actual_length: i32,
    start_frame: i32,
    number_of_packets: i32,
    error_count: i32,
    signr: u32,
    usercontext: *mut libc::c_void,
}

#[cfg(feature = "isochronous")]
#[repr(C)]
struct UsbdevfsIsoUrb {
    header: UsbdevfsUrbHeader,
    iso_frame_desc: [UsbdevfsIsoPacketDesc; MAX_ISO_PACKETS],
}

#[cfg(feature = "isochronous")]
impl Default for UsbdevfsIsoUrb {
    fn default() -> Self {
        Self {
            header: UsbdevfsUrbHeader::default(),
            iso_frame_desc: [UsbdevfsIsoPacketDesc::default(); MAX_ISO_PACKETS],
        }
    }
}

// ioctl number: USBDEVFS_CONTROL = _IOWR('U', 0, struct usbdevfs_ctrltransfer)
// size of struct = 2+1+1+2+2+2+4+ptr = varies by pointer width; use libc constant.
// We construct the ioctl number at runtime to avoid hard-coding architecture-specific values.
fn ioctl_usbdevfs_control(
    fd: libc::c_int,
    transfer: &mut UsbdevfsCtrltransfer,
) -> nix::Result<libc::c_int> {
    // SAFETY: transfer is a valid pointer; fd is a valid usbdevfs fd.
    unsafe {
        let nr = request_code_readwrite!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_CONTROL_NR,
            std::mem::size_of::<UsbdevfsCtrltransfer>()
        );
        let ret = libc::ioctl(fd, nr, transfer as *mut UsbdevfsCtrltransfer);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(ret)
        }
    }
}

fn ioctl_usbdevfs_bulk(
    fd: libc::c_int,
    transfer: &mut UsbdevfsBulktransfer,
) -> nix::Result<libc::c_int> {
    // SAFETY: transfer is a valid pointer; fd is a valid usbdevfs fd.
    unsafe {
        let nr = request_code_readwrite!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_BULK_NR,
            std::mem::size_of::<UsbdevfsBulktransfer>()
        );
        let ret = libc::ioctl(fd, nr, transfer as *mut UsbdevfsBulktransfer);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(ret)
        }
    }
}

fn ioctl_set_interface(fd: libc::c_int, setintf: &UsbdevfsSetinterface) -> nix::Result<()> {
    // USBDEVFS_SETINTERFACE = _IOR('U', 4, struct usbdevfs_setinterface)
    // SAFETY: pointer is valid for the duration of the call.
    unsafe {
        let nr = request_code_read!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_SETINTERFACE_NR,
            std::mem::size_of::<UsbdevfsSetinterface>()
        );
        let ret = libc::ioctl(fd, nr, setintf as *const UsbdevfsSetinterface);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn ioctl_claim_interface(fd: libc::c_int, iface: u32) -> nix::Result<()> {
    unsafe {
        let nr = request_code_readwrite!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_CLAIMINTERFACE_NR,
            std::mem::size_of::<libc::c_uint>()
        );
        let ret = libc::ioctl(fd, nr, &iface as *const u32);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn ioctl_release_interface(fd: libc::c_int, iface: u32) -> nix::Result<()> {
    unsafe {
        let nr = request_code_readwrite!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_RELEASEINTERFACE_NR,
            std::mem::size_of::<libc::c_uint>()
        );
        let ret = libc::ioctl(fd, nr, &iface as *const u32);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn ioctl_reset_endpoint(fd: libc::c_int, endpoint: u32) -> nix::Result<()> {
    // USBDEVFS_RESETEP = _IOR('U', 3, unsigned int)
    // SAFETY: endpoint pointer is valid for the duration of the call.
    unsafe {
        let nr = request_code_read!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_RESETEP_NR,
            std::mem::size_of::<libc::c_uint>()
        );
        let ret = libc::ioctl(fd, nr, &endpoint as *const u32);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn ioctl_clear_halt(fd: libc::c_int, endpoint: u32) -> nix::Result<()> {
    // USBDEVFS_CLEAR_HALT = _IOR('U', 21, unsigned int)
    // SAFETY: endpoint pointer is valid for the duration of the call.
    unsafe {
        let nr = request_code_read!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_CLEAR_HALT_NR,
            std::mem::size_of::<libc::c_uint>()
        );
        let ret = libc::ioctl(fd, nr, &endpoint as *const u32);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn ioctl_reset_device(fd: libc::c_int) -> nix::Result<()> {
    // USBDEVFS_RESET = _IO('U', 20)
    // SAFETY: fd is a valid usbdevfs device file descriptor.
    unsafe {
        let nr = request_code_none!(USBDEVFS_CONTROL_IOCTL, USBDEVFS_RESET_NR);
        let ret = libc::ioctl(fd, nr);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "isochronous")]
fn ioctl_submit_urb(fd: libc::c_int, urb: *mut UsbdevfsUrbHeader) -> nix::Result<()> {
    // USBDEVFS_SUBMITURB = _IOR('U', 10, struct usbdevfs_urb)
    // Request code size is the header size, even for ISO URBs that append
    // `iso_frame_desc[]` entries in trailing memory.
    // SAFETY: `urb` points to valid user memory for the lifetime of the ioctl.
    unsafe {
        let nr = request_code_read!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_SUBMITURB_NR,
            std::mem::size_of::<UsbdevfsUrbHeader>()
        );
        let ret = libc::ioctl(fd, nr, urb);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "isochronous")]
fn ioctl_reap_urb(fd: libc::c_int, out_urb: &mut *mut libc::c_void) -> nix::Result<()> {
    // USBDEVFS_REAPURB = _IOW('U', 12, void *)
    // SAFETY: out_urb points to writable pointer storage.
    unsafe {
        let nr = request_code_write!(
            USBDEVFS_CONTROL_IOCTL,
            USBDEVFS_REAPURB_NR,
            std::mem::size_of::<*mut libc::c_void>()
        );
        let ret = libc::ioctl(fd, nr, out_urb as *mut *mut libc::c_void);
        if ret < 0 {
            Err(nix::errno::Errno::last())
        } else {
            Ok(())
        }
    }
}

fn map_errno_code(errno: i32) -> UsbError {
    match errno {
        libc::EPIPE => UsbError::Stall,
        libc::ETIMEDOUT => UsbError::Timeout,
        libc::ENODEV => UsbError::DeviceNotFound,
        libc::EACCES | libc::EPERM => UsbError::PermissionDenied,
        other => UsbError::Other(format!("errno {other}")),
    }
}

fn map_errno(errno: nix::errno::Errno) -> UsbError {
    map_errno_code(errno as i32)
}

// -----------------------------------------------------------------------
// Public backend entry point
// -----------------------------------------------------------------------

pub struct LinuxBackend;

impl UsbBackend for LinuxBackend {
    fn enumerate(&self) -> Result<Vec<DeviceInfo>, UsbError> {
        enumerate_udev_devices()
    }

    fn open(&self, path: &str) -> Result<Box<dyn UsbDevice>, UsbError> {
        let dev = LinuxDevice::open(path)?;
        Ok(Box::new(dev))
    }
}

// -----------------------------------------------------------------------
// Device enumeration via udev
// -----------------------------------------------------------------------

fn enumerate_udev_devices() -> Result<Vec<DeviceInfo>, UsbError> {
    let mut enumerator = udev::Enumerator::new().map_err(|e| UsbError::Io(e))?;
    enumerator
        .match_subsystem("usb")
        .map_err(|e| UsbError::Io(e))?;
    enumerator
        .match_property("DEVTYPE", "usb_device")
        .map_err(|e| UsbError::Io(e))?;

    let devices = enumerator.scan_devices().map_err(|e| UsbError::Io(e))?;

    let mut result = Vec::new();

    for udev_device in devices {
        // devnode is the /dev/bus/usb/BBB/DDD path
        let path = match udev_device.devnode() {
            Some(p) => p.to_string_lossy().to_string(),
            None => continue,
        };

        let vendor_id = parse_hex_attr(&udev_device, "idVendor");
        let product_id = parse_hex_attr(&udev_device, "idProduct");

        let bus_number = udev_device
            .attribute_value("busnum")
            .and_then(|v| v.to_str())
            .and_then(|s| s.trim().parse::<u8>().ok())
            .unwrap_or(0);

        let device_address = udev_device
            .attribute_value("devnum")
            .and_then(|v| v.to_str())
            .and_then(|s| s.trim().parse::<u8>().ok())
            .unwrap_or(0);

        // String attributes from udev (may not always be populated)
        let manufacturer = udev_device
            .attribute_value("manufacturer")
            .map(|v| v.to_string_lossy().to_string());
        let product = udev_device
            .attribute_value("product")
            .map(|v| v.to_string_lossy().to_string());
        let serial_number = udev_device
            .attribute_value("serial")
            .map(|v| v.to_string_lossy().to_string());

        result.push(DeviceInfo {
            vendor_id,
            product_id,
            bus_number,
            device_address,
            path,
            manufacturer,
            product,
            serial_number,
        });
    }

    Ok(result)
}

fn parse_hex_attr(dev: &udev::Device, attr: &str) -> u16 {
    dev.attribute_value(attr)
        .and_then(|v| v.to_str())
        .map(|s| s.trim())
        .and_then(|s| u16::from_str_radix(s, 16).ok())
        .unwrap_or(0)
}

// -----------------------------------------------------------------------
// LinuxDevice — wraps a usbdevfs file descriptor
// -----------------------------------------------------------------------

struct LinuxDevice {
    file: File,
    endpoint_cache: Mutex<std::collections::HashMap<u8, EndpointInfo>>,
}

impl LinuxDevice {
    /// Open a device by its /dev/bus/usb/BBB/DDD path.
    ///
    /// First tries read+write; falls back to read-only (descriptor reads still work).
    fn open(path: &str) -> Result<Self, UsbError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .or_else(|_| {
                // Fallback: read-only — descriptor reads work, host-to-device OUT transfers will fail.
                log::warn!("usbdevfs: read+write open failed for {path}; retrying read-only");
                OpenOptions::new().read(true).open(path)
            })
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    UsbError::PermissionDenied
                } else {
                    UsbError::Io(e)
                }
            })?;

        let dev = Self {
            file,
            endpoint_cache: Mutex::new(std::collections::HashMap::new()),
        };

        // Best-effort cache warmup for get_pipe_info / isoch packet sizing.
        if let Err(e) = dev.refresh_endpoint_cache() {
            log::debug!("linux: endpoint cache warmup failed: {e}");
        }

        Ok(dev)
    }

    fn raw_control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        buf: &mut [u8],
        timeout_ms: u32,
    ) -> Result<usize, UsbError> {
        let mut transfer = UsbdevfsCtrltransfer {
            request_type,
            request,
            value,
            index,
            length: buf.len() as u16,
            timeout: timeout_ms,
            data: buf.as_mut_ptr() as *mut libc::c_void,
        };

        let fd = self.file.as_raw_fd();
        let n = ioctl_usbdevfs_control(fd, &mut transfer).map_err(map_errno)?;

        Ok(n as usize)
    }

    fn raw_bulk(&self, endpoint: u8, buf: &mut [u8], timeout_ms: u32) -> Result<usize, UsbError> {
        if buf.len() > u32::MAX as usize {
            return Err(UsbError::Other("bulk transfer buffer too large".into()));
        }

        let mut transfer = UsbdevfsBulktransfer {
            ep: endpoint as u32,
            len: buf.len() as u32,
            timeout: timeout_ms,
            data: buf.as_mut_ptr() as *mut libc::c_void,
        };

        let fd = self.file.as_raw_fd();
        let n = ioctl_usbdevfs_bulk(fd, &mut transfer).map_err(map_errno)?;
        Ok(n as usize)
    }

    fn read_config_descriptor_impl(&self, index: u8) -> Result<ConfigDescriptor, UsbError> {
        // First pass: read 9-byte header to get wTotalLength
        let mut hdr = [0u8; 9];
        let req_value = (0x02u16 << 8) | index as u16;
        let n = self.raw_control(0x80, 0x06, req_value, 0x0000, &mut hdr, 5000)?;
        if n < 9 {
            return Err(UsbError::InvalidDescriptor);
        }
        let total_len = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
        if total_len < 9 {
            return Err(UsbError::InvalidDescriptor);
        }

        // Second pass: read full descriptor
        let mut full = vec![0u8; total_len];
        self.raw_control(0x80, 0x06, req_value, 0x0000, &mut full, 5000)?;
        ConfigDescriptor::from_bytes(&full)
    }

    fn refresh_endpoint_cache(&self) -> Result<(), UsbError> {
        let cfg = self.read_config_descriptor_impl(0)?;
        let mut map = std::collections::HashMap::<u8, EndpointInfo>::new();

        for iface in &cfg.interfaces {
            for ep in &iface.endpoints {
                map.insert(
                    ep.endpoint_address,
                    EndpointInfo::new(
                        ep.endpoint_address,
                        ep.attributes,
                        ep.max_packet_size,
                        ep.interval,
                    ),
                );
            }
        }

        let mut guard = self
            .endpoint_cache
            .lock()
            .map_err(|_| UsbError::Other("endpoint cache lock poisoned".into()))?;
        *guard = map;
        Ok(())
    }

    #[cfg(feature = "isochronous")]
    fn isoch_transfer(&self, endpoint: u8, data: *mut libc::c_void, len: usize) -> Result<usize, UsbError> {
        if len == 0 {
            return Ok(0);
        }
        if len > i32::MAX as usize {
            return Err(UsbError::Other("isochronous transfer buffer too large".into()));
        }

        let packet_hint = self
            .get_pipe_info(endpoint)
            .ok()
            .map(|i| i.max_packet_size as usize)
            .filter(|s| *s > 0)
            .unwrap_or(1024);

        let num_packets = ((len + packet_hint - 1) / packet_hint).max(1);
        if num_packets > MAX_ISO_PACKETS {
            return Err(UsbError::Other(format!(
                "isochronous transfer requires {num_packets} packets; max supported is {MAX_ISO_PACKETS}"
            )));
        }

        let mut urb = UsbdevfsIsoUrb::default();
        urb.header.urb_type = USBDEVFS_URB_TYPE_ISO;
        urb.header.endpoint = endpoint;
        urb.header.buffer = data;
        urb.header.buffer_length = len as i32;
        urb.header.number_of_packets = num_packets as i32;

        let base = len / num_packets;
        let rem = len % num_packets;
        for i in 0..num_packets {
            let packet_len = base + usize::from(i < rem);
            urb.iso_frame_desc[i].length = packet_len as u32;
        }

        let fd = self.file.as_raw_fd();
        let header_ptr = &mut urb.header as *mut UsbdevfsUrbHeader;
        urb.header.usercontext = header_ptr.cast::<libc::c_void>();

        ioctl_submit_urb(fd, header_ptr).map_err(map_errno)?;

        let mut reaped: *mut libc::c_void = std::ptr::null_mut();
        ioctl_reap_urb(fd, &mut reaped).map_err(map_errno)?;

        let expected = header_ptr.cast::<libc::c_void>();
        if !reaped.is_null() && reaped != expected {
            return Err(UsbError::Other("reaped unexpected URB pointer".into()));
        }

        if urb.header.status != 0 {
            let code = urb.header.status.abs();
            return Err(map_errno_code(code));
        }

        for frame in urb
            .iso_frame_desc
            .iter()
            .take(num_packets)
            .filter(|d| d.status != 0)
        {
            let code = (frame.status as i32).abs();
            return Err(map_errno_code(code));
        }

        Ok(urb.header.actual_length.max(0) as usize)
    }
}

impl UsbDevice for LinuxDevice {
    fn read_device_descriptor(&self) -> Result<DeviceDescriptor, UsbError> {
        let mut buf = [0u8; 18];
        // GET_DESCRIPTOR: type=0x01 (Device), index=0, lang=0
        let n = self.raw_control(0x80, 0x06, 0x0100, 0x0000, &mut buf, 5000)?;
        if n < 18 {
            return Err(UsbError::InvalidDescriptor);
        }
        DeviceDescriptor::from_bytes(&buf)
    }

    fn read_config_descriptor(&self, index: u8) -> Result<ConfigDescriptor, UsbError> {
        self.read_config_descriptor_impl(index)
    }

    fn read_string_descriptor(&self, index: u8, lang: u16) -> Result<String, UsbError> {
        let mut buf = [0u8; 255];
        let req_value = (0x03u16 << 8) | index as u16;
        let n = self.raw_control(0x80, 0x06, req_value, lang, &mut buf, 5000)?;
        if n < 2 {
            return Err(UsbError::InvalidDescriptor);
        }
        let str_len = buf[0] as usize;
        if str_len < 2 || str_len > n {
            return Err(UsbError::InvalidDescriptor);
        }
        // String content starts at byte 2, UTF-16LE
        let chars: Vec<u16> = buf[2..str_len]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&chars).to_string())
    }

    fn claim_interface(&mut self, interface: u8) -> Result<(), UsbError> {
        let fd = self.file.as_raw_fd();
        ioctl_claim_interface(fd, interface as u32).map_err(|errno| match errno {
            nix::errno::Errno::EBUSY => UsbError::Other("interface already claimed".into()),
            nix::errno::Errno::ENODEV => UsbError::DeviceNotFound,
            other => UsbError::Other(other.to_string()),
        })
    }

    fn release_interface(&mut self, interface: u8) -> Result<(), UsbError> {
        let fd = self.file.as_raw_fd();
        ioctl_release_interface(fd, interface as u32).map_err(|errno| match errno {
            nix::errno::Errno::ENODEV => UsbError::DeviceNotFound,
            other => UsbError::Other(other.to_string()),
        })
    }

    fn control_transfer(
        &self,
        setup: ControlSetup,
        data: Option<&mut [u8]>,
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;

        // If direction is IN (bit 7 of request_type set) we need a receive buffer.
        // If direction is OUT, we send data from the caller's slice.
        let is_in = (setup.request_type & 0x80) != 0;

        if is_in {
            let buf =
                data.ok_or_else(|| UsbError::Other("IN transfer requires a data buffer".into()))?;
            self.raw_control(
                setup.request_type,
                setup.request,
                setup.value,
                setup.index,
                buf,
                timeout_ms,
            )
        } else {
            // OUT transfer — if caller supplies data, use it; otherwise send zero bytes.
            let len = setup.length as usize;
            match data {
                Some(buf) => self.raw_control(
                    setup.request_type,
                    setup.request,
                    setup.value,
                    setup.index,
                    buf,
                    timeout_ms,
                ),
                None => {
                    let mut empty = vec![0u8; len];
                    self.raw_control(
                        setup.request_type,
                        setup.request,
                        setup.value,
                        setup.index,
                        &mut empty,
                        timeout_ms,
                    )
                }
            }
        }
    }

    fn bulk_read(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        if endpoint & 0x80 == 0 {
            return Err(UsbError::Other(format!(
                "bulk_read requires an IN endpoint, got {endpoint:#04x}"
            )));
        }
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        self.raw_bulk(endpoint, buf, timeout_ms)
    }

    fn bulk_write(&self, endpoint: u8, buf: &[u8], timeout: Duration) -> Result<usize, UsbError> {
        if endpoint & 0x80 != 0 {
            return Err(UsbError::Other(format!(
                "bulk_write requires an OUT endpoint, got {endpoint:#04x}"
            )));
        }
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;

        // usbdevfs bulk ioctl takes a mutable pointer regardless of direction.
        // Copy into a temporary mutable buffer for a safe OUT transfer.
        let mut tmp = buf.to_vec();
        self.raw_bulk(endpoint, &mut tmp, timeout_ms)
    }

    fn interrupt_read(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        // usbdevfs uses the same bulk-style synchronous ioctl path here.
        self.bulk_read(endpoint, buf, timeout)
    }

    fn interrupt_write(
        &self,
        endpoint: u8,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        self.bulk_write(endpoint, buf, timeout)
    }

    fn reset_pipe(&self, endpoint: u8) -> Result<(), UsbError> {
        let fd = self.file.as_raw_fd();
        let ep = endpoint as u32;

        // Try CLEAR_HALT first (preferred), then RESETEP for older kernels.
        match ioctl_clear_halt(fd, ep) {
            Ok(()) => Ok(()),
            Err(nix::errno::Errno::ENOTTY | nix::errno::Errno::EINVAL) => {
                ioctl_reset_endpoint(fd, ep).map_err(map_errno)
            }
            Err(e) => Err(map_errno(e)),
        }
    }

    fn abort_pipe(&self, endpoint: u8) -> Result<(), UsbError> {
        // usbdevfs synchronous transfers don't expose a per-pipe "abort pending URBs"
        // operation without tracking submitted async URBs. Best-effort fallback: clear
        // the halt/reset state so subsequent I/O can proceed.
        self.reset_pipe(endpoint)
    }

    fn reset_device(&self) -> Result<(), UsbError> {
        let fd = self.file.as_raw_fd();
        ioctl_reset_device(fd).map_err(map_errno)
    }

    fn get_alternate_setting(&self, interface: u8) -> Result<u8, UsbError> {
        // GET_INTERFACE (IN | Standard | Interface)
        let mut buf = [0u8; 1];
        let setup = ControlSetup {
            request_type: 0x81,
            request: 0x0A,
            value: 0,
            index: interface as u16,
            length: 1,
        };
        let n = self.control_transfer(setup, Some(&mut buf), Duration::from_millis(1000))?;
        if n < 1 {
            return Err(UsbError::InvalidDescriptor);
        }
        Ok(buf[0])
    }

    fn set_alternate_setting(&mut self, interface: u8, alt: u8) -> Result<(), UsbError> {
        let fd = self.file.as_raw_fd();
        let setintf = UsbdevfsSetinterface {
            interface: interface as u32,
            altsetting: alt as u32,
        };
        ioctl_set_interface(fd, &setintf).map_err(map_errno)?;
        // Keep cached endpoint info in sync with the new alternate setting.
        if let Err(e) = self.refresh_endpoint_cache() {
            log::debug!("linux: endpoint cache refresh after set_interface failed: {e}");
        }
        Ok(())
    }

    fn get_pipe_info(&self, endpoint: u8) -> Result<EndpointInfo, UsbError> {
        {
            let guard = self
                .endpoint_cache
                .lock()
                .map_err(|_| UsbError::Other("endpoint cache lock poisoned".into()))?;
            if let Some(info) = guard.get(&endpoint) {
                return Ok(info.clone());
            }
        }

        // Cache miss: attempt one refresh and retry.
        self.refresh_endpoint_cache()?;
        let guard = self
            .endpoint_cache
            .lock()
            .map_err(|_| UsbError::Other("endpoint cache lock poisoned".into()))?;
        guard
            .get(&endpoint)
            .cloned()
            .ok_or_else(|| UsbError::Other(format!("endpoint {endpoint:#04x} not found")))
    }

    fn get_pipe_policy(
        &self,
        _endpoint: u8,
        _kind: PipePolicyKind,
    ) -> Result<PipePolicy, UsbError> {
        // usbdevfs does not expose WinUSB-style pipe policy controls.
        Err(UsbError::Unsupported)
    }

    fn set_pipe_policy(&self, _endpoint: u8, _policy: PipePolicy) -> Result<(), UsbError> {
        // usbdevfs does not expose WinUSB-style pipe policy controls.
        Err(UsbError::Unsupported)
    }

    fn async_bulk_read(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        // Linux backend currently provides timeout-based synchronous I/O here.
        self.bulk_read(endpoint, buf, timeout)
    }

    fn async_bulk_write(
        &self,
        endpoint: u8,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        self.bulk_write(endpoint, buf, timeout)
    }

    fn async_interrupt_read(
        &self,
        endpoint: u8,
        buf: &mut [u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        self.interrupt_read(endpoint, buf, timeout)
    }

    fn async_interrupt_write(
        &self,
        endpoint: u8,
        buf: &[u8],
        timeout: Duration,
    ) -> Result<usize, UsbError> {
        self.interrupt_write(endpoint, buf, timeout)
    }

    #[cfg(feature = "isochronous")]
    fn isoch_read(&self, endpoint: u8, buf: &mut [u8]) -> Result<usize, UsbError> {
        if endpoint & 0x80 == 0 {
            return Err(UsbError::Other(format!(
                "isoch_read requires an IN endpoint, got {endpoint:#04x}"
            )));
        }
        self.isoch_transfer(endpoint, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len())
    }

    #[cfg(feature = "isochronous")]
    fn isoch_write(&self, endpoint: u8, buf: &[u8]) -> Result<usize, UsbError> {
        if endpoint & 0x80 != 0 {
            return Err(UsbError::Other(format!(
                "isoch_write requires an OUT endpoint, got {endpoint:#04x}"
            )));
        }
        self.isoch_transfer(endpoint, buf.as_ptr().cast_mut().cast::<libc::c_void>(), buf.len())
    }
}
