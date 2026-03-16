# usblibr Progress Report

Date: 2026-03-16  
Repository: `rustusblib`  
Branch: `main`

## Analysis Method (Sub-Agent Style)

To analyze the codebase end-to-end, the review was split into focused "sub-agents":

1. History agent: analyzed commit timeline and milestone sequence.
2. API agent: audited `usb-lib/src/api/*` and public exports in `usb-lib/src/lib.rs`.
3. Windows agent: audited `usb-lib/src/backend/windows.rs` feature coverage and build behavior.
4. Linux agent: audited `usb-lib/src/backend/linux.rs` implementation depth.
5. macOS agent: audited `usb-lib/src/backend/macos.rs` implementation depth.
6. Validation agent: ran build/test/lint commands and captured current blockers.
7. Docs agent: compared README claims against the actual API and backend behavior.

## End-to-End Timeline (Start to Current)

1. `97b4d5b` - Scaffold created: crate layout, core types, API wrapper, error model.
2. `22db110` - Windows backend: enumeration and open path implemented.
3. `b14c47d` - Descriptor models/parsers: device/config/string/HID/BOS/hub/superspeed companion.
4. `1bcc204` - Control transfer support and builder helpers.
5. `20405c5` - Bulk and interrupt transfer support.
6. `af695d1` - Pipe management: query/policy/reset/abort.
7. `62e07af` - Interface management and endpoint cache.
8. `23d0dcc` - Device reset via manual FFI binding.
9. `488f49f` - Overlapped async transfer methods.
10. `7e58bf9` - Hotplug support added.
11. `413ef70` - `.gitignore` update.
12. `cdd63de` - Linux backend introduced (udev + `USBDEVFS_CONTROL`).
13. `3332c0b` - macOS backend introduced (IOKit + IOUSBDevice interface path).
14. `9c79627` - Isochronous transfers behind feature flag.
15. `69aeadf` - Tokio integration behind feature flag.
16. `61934f2` - `DEVELOPMENT.md` added.
17. `6e0cef6` - README/docs commit.
18. `f056b4b` - ignore updates.
19. `0ef15dc` - HRESULT mapping and lint cleanup fixes.

## Verified Build and Validation Status (Today)

Commands run on 2026-03-16:

- `cargo check` -> pass
- `cargo check --features tokio` -> pass
- `cargo check --features isochronous` -> pass
- `cargo check --all-features` -> pass
- `cargo clippy --all-features -- -D warnings` -> pass
- `cargo test --lib` -> pass (0 tests present)
- `cargo test` -> fail (example link failure)
- `cargo build --examples` -> fail (same link failure)

Current hard blocker:

- Windows GNU link fails with `undefined reference to WinUsb_ResetDevice` from `windows.rs` when examples/tests link.
- macOS path parser expects `bus=` but enumeration emits `iokit:bus=...`, so `open()` from enumerated path is likely broken.

Cross-platform compile validation limits on this machine:

- Installed targets: `x86_64-pc-windows-gnu`, `x86_64-unknown-linux-gnu`.
- Linux cross-check currently fails at `libudev-sys` build step because cross `pkg-config` sysroot/tooling is not configured on this Windows host.
- `cargo check --target x86_64-apple-darwin` still requires adding that target first.

## Platform Capability Matrix (Implemented vs Missing)

| Capability | Windows | Linux | macOS | Status Notes |
|---|---|---|---|---|
| Device enumeration | Implemented | Implemented | Implemented | Native APIs used per OS |
| Device open/close | Implemented | Implemented | Implemented | macOS uses IOUSB interface plugin path |
| Device descriptor read | Implemented | Implemented | Implemented | |
| Config descriptor read | Implemented | Implemented | Implemented | two-pass in all backends |
| String descriptor read | Implemented | Implemented | Implemented | |
| Generic control transfer | Implemented | Implemented | Implemented | |
| Interface claim/release | Implemented | Implemented | Partial | macOS now validates + tracks claims via GET/SET_INTERFACE control path |
| Bulk transfers | Implemented | Implemented | Missing | Linux uses `USBDEVFS_BULK` |
| Interrupt transfers | Implemented | Implemented | Missing | Linux currently reuses the usbdevfs bulk-style path |
| Pipe reset/abort | Implemented | Implemented | Missing | Linux uses `USBDEVFS_CLEAR_HALT`/`USBDEVFS_RESETEP` |
| Alt setting get/set | Implemented | Implemented | Missing | Linux uses GET_INTERFACE + `USBDEVFS_SETINTERFACE` |
| Pipe policy get/set | Implemented | Kernel-limited (`Unsupported`) | Missing | usbdevfs has no WinUSB-style pipe policy controls |
| Pipe info query | Implemented | Missing | Missing | |
| Device reset | Implemented (code) | Implemented | Missing | Windows still has a link blocker in example/test link |
| BOS descriptor read | Implemented | Missing | Missing | Linux/macOS not overridden |
| Hub descriptor read | Implemented | Missing | Missing | Linux/macOS not overridden |
| Async bulk/interrupt methods | Implemented | Implemented | Missing | Linux async methods currently map to timeout-based sync I/O |
| Isochronous transfer | Implemented (feature gated) | Implemented (feature gated) | Missing | Linux uses `USBDEVFS_SUBMITURB` + `USBDEVFS_REAPURB` |
| Hotplug callback | Implemented | Implemented | Missing | Linux uses udev monitor socket on a worker thread |

### Linux Snapshot

| Feature | Linux |
|---|---|
| Bulk/interrupt transfers | Implemented |
| Pipe reset/abort | Implemented |
| Async transfer path | Implemented (sync-backed) |
| Hotplug | Implemented |
| Interface claim/release | Implemented |
| Control transfer | Implemented |
| Descriptors | Implemented |
| Alt-setting get/set | Implemented |
| Pipe info | Implemented |
| Pipe policy | Kernel-limited (`Unsupported`) |
| Isochronous | Implemented (feature-gated) |

## What Has Been Added (Complete Inventory)

Core and API:

- Public context + device handle abstraction over `dyn UsbDevice`.
- Rich descriptor parsing module (`Device`, `Config`, `Interface`, `Endpoint`, `HID`, `BOS`, `Hub`, `SS Companion`).
- Control setup helper constructors.
- Pipe policy enums and endpoint info models.
- Error enum with Windows HRESULT mapping.

Windows backend:

- WinUSB enumeration via SetupAPI and interface GUID filtering.
- Open path with read-write and read-only fallback.
- Control, bulk, interrupt transfers.
- Interface claim/release plus endpoint-to-interface cache.
- Pipe reset/abort, alt setting get/set, pipe info query, pipe policy get/set.
- Device reset via manual FFI (`WinUsb_ResetDevice`).
- Async transfer paths via OVERLAPPED and wait-based completion.
- Feature-gated isochronous transfer helpers via manual FFI.
- String descriptor enrichment during enumeration.

Linux backend:

- udev enumeration.
- Open `/dev/bus/usb/...` with read-write/read-only fallback.
- `USBDEVFS_CONTROL`-based control transfer path.
- Interface claim/release via usbdevfs ioctls.
- Device/config/string descriptor read support.
- Bulk and interrupt transfers via `USBDEVFS_BULK`.
- Pipe reset and abort via `USBDEVFS_CLEAR_HALT` / `USBDEVFS_RESETEP`.
- Device reset via `USBDEVFS_RESET`.
- Alt-setting get/set via GET_INTERFACE + `USBDEVFS_SETINTERFACE`.
- Pipe info via endpoint descriptor cache.
- Pipe policy explicitly returns `Unsupported` (kernel limitation).
- Feature-gated isochronous support via `USBDEVFS_SUBMITURB` / `USBDEVFS_REAPURB`.
- Async transfer methods implemented as timeout-based sync wrappers.

macOS backend:

- IOKit enumeration over `IOUSBDevice` services.
- IOUSB device interface acquisition via plugin `QueryInterface`.
- Control transfers through `DeviceRequestTO`.
- Device/config/string descriptor read support.
- `parse_iokit_path` fixed for `iokit:bus=...,addr=...` paths.
- Interface claim/release now implemented as non-no-op via interface validation + GET/SET_INTERFACE and claim tracking.

Async/hotplug:

- Tokio wrappers (feature-gated) around async transfer methods.
- Hotplug implementation on Windows via `CM_Register_Notification`.
- Linux hotplug implementation via `udev::MonitorBuilder` + poll loop worker thread.
- Non-Windows/non-Linux hotplug remains `Unsupported`.

## Gaps and Work Still Needed

### Project-wide (P0)

1. Fix Windows GNU link issue for `WinUsb_ResetDevice` so examples/tests link cleanly.
2. Add CI matrix for Windows/Linux/macOS compile checks.
3. Add real tests (descriptor parser unit tests, backend behavior tests, API smoke tests).
4. Sync README with actual API names and behavior.
5. Add missing referenced docs/files (`CONTRIBUTING.md`, `LICENSE`) or update README links.

### Windows (P0/P1)

1. Resolve `WinUsb_ResetDevice` link reliability across toolchains.
2. Add test coverage for advanced operations (pipe policy, alt settings, async, isoch).
3. Add hardware-backed integration test guide and compatibility matrix.

### Linux (P0/P1)

1. Implement BOS/hub descriptor convenience methods (currently trait default).
2. Add native async URB path if true non-blocking semantics are required (current async methods are sync-backed).

### macOS (P0/P1)

1. Implement bulk/interrupt transfers via interface/pipe APIs.
2. Implement alt-setting controls and pipe-level controls.
3. Implement BOS/hub descriptor convenience methods.
4. Implement async transfer path (or explicit unsupported contract).
5. Implement macOS hotplug callback path.

### macOS Next Checklist

- [ ] Bulk/interrupt (next)
- [ ] Alt-setting, pipe info/policy
- [ ] Hotplug
- [ ] Isochronous

## Documentation Drift Found

Current README mismatches code behavior and API:

1. Uses `ctx.list_devices()` but API provides `ctx.devices()`.
2. Uses `ctx.open_device(...)` but API provides `ctx.open(...)`.
3. Uses `ControlSetup::new(...)` and `RequestType`/`Recipient` types not present in current code.
4. Hotplug is now implemented on Linux but still unsupported on macOS.
5. Example import path uses `usblibr` while library exports as `usb_lib` in code.
6. Tokio wrapper docs mention `spawn_blocking`, but implementation uses `block_in_place`.

## Recommended Completion Sequence

1. Stabilize Windows build/link (unblocks example and full test builds).
2. Fix README/docs to match current API exactly.
3. Add cross-platform CI compile gates.
4. Validate Linux backend on a native Linux runner (real hardware + hotplug + transfer smoke tests).
5. Deliver macOS bulk/interrupt + proper interface handling + hotplug.
6. Add comprehensive automated tests after backend parity improves.

## Completion Definition (Done Criteria)

The project can be marked "cross-platform MVP complete" when all are true:

1. Windows/Linux/macOS each support enumeration, open, descriptors, control, bulk, interrupt.
2. Hotplug works on all three platforms.
3. README/API/docs are consistent and runnable.
4. CI passes on all three OSes.
5. At least smoke-level tests exist for all public API groups.
6. Examples build and run without linker/runtime blockers.
