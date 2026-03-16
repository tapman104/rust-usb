# Development Notes

Architectural decisions made during implementation of `usb-lib`.

---

## WinUSB GUID is a compile-time `const`

```rust
const WINUSB_DEVICE_INTERFACE_GUID: windows::core::GUID = windows::core::GUID {
    data1: 0xDEE824EF, ...
};
```

The WinUSB device interface GUID `{DEE824EF-729B-4A0E-9C14-B7117D33A817}` is
fixed by the WinUSB driver and never changes. Hardcoding it as a `const`
eliminates any runtime lookup, avoids a dependency on
`SetupDiClassGuidsFromName` or registry reads, and makes enumeration
deterministic. The GUID is documented in the Windows Driver Kit and treated
as a stable ABI contract.

---

## `CreateFileW` opens read+write with a read-only fallback

```rust
// Attempt 1: GENERIC_READ | GENERIC_WRITE
// Attempt 2 (on ERROR_ACCESS_DENIED): GENERIC_READ only
```

Some devices or driver configurations deny `GENERIC_WRITE` to non-elevated
processes (e.g. HID-composite devices, devices opened by another process).
Retrying with `GENERIC_READ` alone still allows `WinUsb_Initialize` to
succeed and descriptor reads to work — control transfers in the host-to-device
direction will fail at the OS level, but device-to-host reads remain fully
functional. Without the fallback, those devices appear inaccessible when they
are not.

---

## `WinUsb_ResetDevice` and isochronous APIs are bound via manual FFI

```rust
extern "system" {
    fn WinUsb_ResetDevice(InterfaceHandle: *mut c_void) -> BOOL;
    fn WinUsb_RegisterIsochBuffer(...) -> BOOL;
    ...
}
```

`windows-rs 0.58` does not expose `WinUsb_ResetDevice`,
`WinUsb_RegisterIsochBuffer`, `WinUsb_UnregisterIsochBuffer`,
`WinUsb_ReadIsochPipeAsap`, or `WinUsb_WriteIsochPipeAsap`. These symbols are
present in `winusb.dll` at runtime; the missing bindings are a gap in the
generated metadata. Manual `extern "system"` blocks work because `winusb.lib`
is already pulled into the link graph by the `windows` crate's
`Win32_Devices_Usb` feature — no extra `#[link]` attribute is required.

---

## Async transfers use `WaitForSingleObject` rather than a pure async mechanism

The `UsbDevice` trait is a regular (non-async) trait used as a `dyn` object:

```rust
pub trait UsbDevice: Send { ... }
```

Placing `async fn` on an object-safe trait requires either the `async-trait`
proc-macro crate (external dependency, heap allocation per call) or Rust's
unstable `return_position_impl_trait_in_trait`. Neither is acceptable for a
stable, zero-overhead library.

Instead, `async_bulk_read` etc. are ordinary `fn` methods that submit an
OVERLAPPED operation and call `WaitForSingleObject` to block the calling
thread until the OS signals completion. The file handle was opened with
`FILE_FLAG_OVERLAPPED` so the OS doesn't block during I/O — only the explicit
`WaitForSingleObject` call blocks, giving callers precise timeout control.

---

## The `tokio` feature uses `block_in_place` rather than `spawn_blocking`

```rust
tokio::task::block_in_place(|| handle.async_bulk_read(endpoint, buf, timeout))
```

`spawn_blocking` requires the closure to be `'static + Send`. `DeviceHandle`
wraps a `Box<dyn UsbDevice>` — the inner device is `Send` but `DeviceHandle`
exposes `&mut self` methods and is not `Clone`, so it cannot be moved into a
`'static` closure without ownership transfer. With `block_in_place` the async
task and the blocking call stay on the same OS thread; no ownership or
lifetime boundary is crossed. The buffer reference stays valid, and the
Tokio scheduler marks the thread as "blocking" for the duration so other tasks
are not starved.

---

## The `tokio` feature requires `rt-multi-thread`

`block_in_place` is only available on Tokio's multi-thread scheduler. On the
`current_thread` scheduler it panics at runtime. Rather than silently compile
and panic later, `Cargo.toml` unconditionally enables `rt-multi-thread` when
the `tokio` feature is active:

```toml
tokio = { version = "1", features = ["rt", "rt-multi-thread", "sync"], optional = true }
```

This makes the constraint visible at dependency resolution time. Applications
that genuinely need single-threaded operation should use the raw
`DeviceHandle::async_bulk_read` methods directly and integrate with their own
wait strategy.

---

## Isochronous support is an opt-in feature flag

```toml
[features]
isochronous = []
```

Isochronous endpoints are used by audio and video class devices, which
represent a small fraction of USB peripherals. The feature is off by default
because:

1. The FFI functions involved (`WinUsb_RegisterIsochBuffer` etc.) are absent
   from the windows-rs bindings and require manual `unsafe` declarations.
2. Every isochronous call involves buffer registration/unregistration and an
   OVERLAPPED round-trip — the additional code bloats compile output for users
   who never need it.
3. Keeping it optional gives the compiler a chance to dead-strip the code and
   keeps the default binary smaller.

Non-Windows targets and builds without the flag return `UsbError::Unsupported`
from the trait default, so callers can handle absence gracefully at runtime
without `#[cfg]` sprawl in application code.

---

## Endpoint-to-handle cache for multi-interface devices

`WinUsbDevice` maintains a `HashMap<u8, WINUSB_INTERFACE_HANDLE>` populated
when each interface is claimed:

```
claim_interface(n)
  └─ WinUsb_GetAssociatedInterface  →  assoc_handle
       └─ build_endpoint_cache(assoc_handle)
            └─ loop WinUsb_QueryPipe(idx = 0..32)
                 └─ endpoint_cache.insert(pipe_id, assoc_handle)
```

`handle_for_endpoint(ep)` checks the cache first (O(1) lookup), then falls
back to a full scan only for endpoints that were never explicitly cached (e.g.
single-interface devices where `claim_interface(0)` was not called). When an
interface is released, all cache entries pointing at its handle are evicted via
`HashMap::retain`.

Without the cache, every bulk/interrupt/isochronous transfer on a
multi-interface device would scan all held handles and probe up to 32 pipe
indices each — O(interfaces × pipes) per transfer.

---

## Two-pass descriptor read pattern

Descriptors of variable length (Configuration, BOS) are read in two passes:

```
Pass 1: GET_DESCRIPTOR with length = header size (9 bytes for Config, 5 for BOS)
        → read wTotalLength from the header
Pass 2: GET_DESCRIPTOR with length = wTotalLength (clamped to 4096)
        → receive the full descriptor blob
```

Allocating the exact required size avoids both under-reading (truncated
descriptors) and over-requesting (some devices NAK or return garbage if
`wLength` exceeds the actual descriptor). The pattern mirrors what the
Windows DDK documentation and usbview.exe use.

---

## Linux uses `USBDEVFS_CONTROL` ioctl; macOS uses the `IOUSBDevice` vtable

Both backends avoid `libusb` intentionally:

**Linux** — USB character devices appear at `/dev/bus/usb/<bus>/<addr>`. The
`USBDEVFS_CONTROL` ioctl (from `<linux/usbdevice_fs.h>`) is the stable,
documented kernel interface for control transfers without a kernel driver.
`udev` provides enumeration. No shared library dependency, no pkg-config,
works inside containers that mount `/dev/bus/usb`.

**macOS** — IOKit exposes `IOUSBDeviceInterface` as a COM-style vtable via
`IOCreatePlugInInterfaceForService`. Calling through the vtable
(`DeviceRequestTO`) is the documented macOS way to issue control transfers
from user space. `IOKit-sys` provides the necessary bindings. Again, no
libusb; the code links directly against `IOKit.framework` and
`CoreFoundation.framework` which are always present on macOS.

Both choices keep the dependency tree minimal and avoid the `libusb` C
library's LGPL license implications for static linking.
