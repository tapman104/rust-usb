# usblibr Code Audit Report

Date: 2026-03-16  
Auditor: Codex (code-first audit)

## Scope Audited

- `usb-lib/src/lib.rs`
- `usb-lib/src/api/mod.rs`
- `usb-lib/src/api/context.rs`
- `usb-lib/src/api/device_handle.rs`
- `usb-lib/src/api/async_transfers.rs`
- `usb-lib/src/backend/windows.rs`
- `usb-lib/src/backend/linux.rs`
- `usb-lib/src/backend/macos.rs`
- `usb-lib/src/hotplug.rs`
- `usb-lib/src/error.rs`
- `usb-lib/Cargo.toml`
- `README.md`
- `progress.md`

## Validation Commands Run

- `cargo check --all-features` (pass)
- `cargo clippy --all-features -- -D warnings` (pass)
- `cargo test --lib` (pass, 0 tests)
- `cargo test` (fail: link error)
- `cargo build --examples` (fail: link error)
- `cargo check --target x86_64-unknown-linux-gnu` (fail: cross pkg-config/libudev setup)
- `cargo check --target x86_64-apple-darwin` (fail: macOS backend compile errors)

## Backend Trait Method Coverage (Evidence-Based)

Legend: `Implemented` = concrete backend body, `Stubbed` = method exists but unsupported/partial semantics, `Missing` = not overridden (falls back to `UsbDevice` default `Err(UsbError::Unsupported)` in `usb-lib/src/backend/mod.rs`).

| UsbDevice trait method | Windows | Linux | macOS |
|---|---|---|---|
| `read_device_descriptor` | Implemented (`windows.rs:666`) | Implemented (`linux.rs:606`) | Implemented (`macos.rs:344`) |
| `read_config_descriptor` | Implemented (`windows.rs:673`) | Implemented (`linux.rs:616`) | Implemented (`macos.rs:360`) |
| `read_string_descriptor` | Implemented (`windows.rs:692`) | Implemented (`linux.rs:620`) | Implemented (`macos.rs:379`) |
| `claim_interface` | Implemented (`windows.rs:718`) | Implemented (`linux.rs:639`) | Stubbed/partial (`macos.rs:397`, only bookkeeping + control probe) |
| `release_interface` | Implemented (`windows.rs:746`) | Implemented (`linux.rs:648`) | Stubbed/partial (`macos.rs:415`, only bookkeeping + SET_INTERFACE alt 0) |
| `control_transfer` | Implemented (`windows.rs:770`) | Implemented (`linux.rs:656`) | Implemented (`macos.rs:427`) |
| `bulk_read` | Implemented (`windows.rs:780`) | Implemented (`linux.rs:706`) | Missing (`backend/mod.rs:59` default) |
| `bulk_write` | Implemented (`windows.rs:798`) | Implemented (`linux.rs:721`) | Missing (`backend/mod.rs:70` default) |
| `interrupt_read` | Implemented (`windows.rs:816`) | Implemented (`linux.rs:735`) | Missing (`backend/mod.rs:80` default) |
| `interrupt_write` | Implemented (`windows.rs:826`) | Implemented (`linux.rs:745`) | Missing (`backend/mod.rs:90` default) |
| `reset_pipe` | Implemented (`windows.rs:835`) | Implemented (`linux.rs:754`) | Missing (`backend/mod.rs:104` default) |
| `abort_pipe` | Implemented (`windows.rs:841`) | Stubbed/partial (`linux.rs:768`, delegates `reset_pipe`) | Missing (`backend/mod.rs:109` default) |
| `reset_device` | Implemented-in-source (`windows.rs:847`) but link-broken | Implemented (`linux.rs:775`) | Missing (`backend/mod.rs:118` default) |
| `get_alternate_setting` | Implemented (`windows.rs:858`) | Implemented (`linux.rs:780`) | Missing (`backend/mod.rs:123` default) |
| `set_alternate_setting` | Implemented (`windows.rs:868`) | Implemented (`linux.rs:797`) | Missing (`backend/mod.rs:128` default) |
| `get_pipe_info` | Implemented (`windows.rs:874`) | Implemented (`linux.rs:811`) | Missing (`backend/mod.rs:138` default) |
| `get_pipe_policy` | Implemented (`windows.rs:906`) | Stubbed (`linux.rs:834`, `Unsupported`) | Missing (`backend/mod.rs:146` default) |
| `set_pipe_policy` | Implemented (`windows.rs:957`) | Stubbed (`linux.rs:843`, `Unsupported`) | Missing (`backend/mod.rs:158` default) |
| `read_bos_descriptor` | Implemented (`windows.rs:991`) | Missing (`backend/mod.rs:169` default) | Missing (`backend/mod.rs:169` default) |
| `read_hub_descriptor` | Implemented (`windows.rs:1008`) | Missing (`backend/mod.rs:176` default) | Missing (`backend/mod.rs:176` default) |
| `async_bulk_read` | Implemented (`windows.rs:1027`) | Stubbed/partial (`linux.rs:848`, sync-backed) | Missing (`backend/mod.rs:190` default) |
| `async_bulk_write` | Implemented (`windows.rs:1037`) | Stubbed/partial (`linux.rs:858`, sync-backed) | Missing (`backend/mod.rs:200` default) |
| `async_interrupt_read` | Implemented (`windows.rs:1047`) | Stubbed/partial (`linux.rs:867`, sync-backed) | Missing (`backend/mod.rs:210` default) |
| `async_interrupt_write` | Implemented (`windows.rs:1057`) | Stubbed/partial (`linux.rs:876`, sync-backed) | Missing (`backend/mod.rs:220` default) |
| `isoch_read` | Implemented with feature flag (`windows.rs:1072`) | Implemented with feature flag (`linux.rs:886`) | Missing (`backend/mod.rs:243` default) |
| `isoch_write` | Implemented with feature flag (`windows.rs:1078`) | Implemented with feature flag (`linux.rs:896`) | Missing (`backend/mod.rs:253` default) |

## FFI Symbol Audit

### Windows backend (`usb-lib/src/backend/windows.rs`)

Manual extern symbols:

- `WinUsb_ResetDevice` (`windows.rs:37`)  
  Status: **Declared, called at `windows.rs:850`, but not link-resolved** on current GNU toolchain (`undefined reference` during example/test link).
- `WinUsb_RegisterIsochBuffer` (`windows.rs:48`)
- `WinUsb_UnregisterIsochBuffer` (`windows.rs:56`)
- `WinUsb_ReadIsochPipeAsap` (`windows.rs:60`)
- `WinUsb_WriteIsochPipeAsap` (`windows.rs:70`)  
  Status: declared behind `isochronous`; not observed failing first because link fails earlier at `WinUsb_ResetDevice`.

Windows crate symbols (SetupAPI/WinUSB/etc):

- Imported from `windows` crate (`windows.rs:4-25`), compile-check succeeds on host target.
- Symbols used in code:
  - SetupAPI: `SetupDiGetClassDevsW`, `SetupDiEnumDeviceInterfaces`, `SetupDiGetDeviceInterfaceDetailW`, `SetupDiGetDeviceRegistryPropertyW`, `SetupDiDestroyDeviceInfoList`
  - WinUSB core: `WinUsb_Initialize`, `WinUsb_Free`, `WinUsb_ControlTransfer`, `WinUsb_ReadPipe`, `WinUsb_WritePipe`
  - Interface/pipe: `WinUsb_GetAssociatedInterface`, `WinUsb_QueryPipe`, `WinUsb_GetCurrentAlternateSetting`, `WinUsb_SetCurrentAlternateSetting`, `WinUsb_GetPipePolicy`, `WinUsb_SetPipePolicy`, `WinUsb_ResetPipe`, `WinUsb_AbortPipe`
  - I/O primitives: `CreateFileW`, `GetOverlappedResult`, `CreateEventW`, `WaitForSingleObject`, `CloseHandle`
  - Status: these symbols are declared via `windows` metadata and compile; unresolved symbol observed is only manual `WinUsb_ResetDevice`.

### Linux backend (`usb-lib/src/backend/linux.rs`)

- No manual extern symbol declarations.
- Uses `libc::ioctl` through Rust wrappers (`linux.rs:120`, `140`, `160`, `178`, `194`, `210`, `228`, `246`, `261`, `282`).
- Linking is via libc; no unresolved symbols observed on host build.
- Effective external symbol usage:
  - `libc::ioctl` (all usbdevfs operations)
  - `libc::poll` in Linux hotplug worker (`hotplug.rs:238`)
  - Status: symbol resolution is fine on host; Linux cross-target build is blocked by `libudev-sys` cross `pkg-config` environment.

### macOS backend (`usb-lib/src/backend/macos.rs`)

Used symbols that fail to resolve in current dependency/API surface:

- `iokit_sys::ret_codes::kIOReturnSuccess` (`macos.rs:14`) - unresolved import.
- `iokit_sys::IOUSBDeviceInterface` (`macos.rs:197`, `543`, `571`) - missing type.
- `iokit_sys::IOUSBDevRequestTO` (`macos.rs:262`) - missing type.
- `IOCreatePlugInInterfaceForService`, `kIOCFPlugInInterfaceID`, `kIOUSBDeviceUserClientTypeID`, `IOCFPlugInInterface` (`macos.rs:545-547`) - unresolved imports.
- `iokit_sys::CFUUIDGetUUIDBytes`, `iokit_sys::kIOUSBDeviceInterfaceID` (`macos.rs:576`) - missing functions.

Also type mismatch:

- `IORegistryEntryCreateCFProperty(..., kCFAllocatorDefault, ...)` uses allocator type that mismatches expected `CFAllocatorRef` in this build (`macos.rs:138`, `159`).
- Symbols that do resolve from `iokit_sys` in this code path:
  - `IOServiceMatching`, `IOServiceGetMatchingServices`, `IOIteratorNext`, `IOObjectRelease`, `IORegistryEntryCreateCFProperty`
  - Status: these compile, but backend still fails overall due missing IOUSBLib symbols/types listed above.

## Explicit `todo!/unimplemented!/panic!/Unsupported` Inventory

- `todo!`: none found in audited files.
- `unimplemented!`: none found in audited files.
- `panic!`: none found in audited files.
- `unreachable!`: `usb-lib/src/backend/windows.rs:951`
- `Err(UsbError::Unsupported)` occurrences:
  - `usb-lib/src/backend/linux.rs:840`
  - `usb-lib/src/backend/linux.rs:845`
  - `usb-lib/src/hotplug.rs:281` (non-Windows/non-Linux)
  - Default trait fallbacks in `usb-lib/src/backend/mod.rs`: lines `65, 76, 86, 96, 105, 110, 119, 124, 129, 139, 151, 159, 170, 177, 196, 206, 216, 226, 244, 254`

## Findings (Ranked by Severity)

### P0

1.
Location: `usb-lib/src/backend/macos.rs:14`, `197`, `262`, `543-547`, `576`, `138`, `159`  
Severity: P0  
Finding: macOS backend does not compile for macOS target.  
Evidence: `cargo check --target x86_64-apple-darwin` reports unresolved `iokit_sys` imports/types/functions and allocator type mismatch in those lines.

2.
Location: `usb-lib/src/backend/windows.rs:37-39`, `850`  
Severity: P0  
Finding: `reset_device` uses manual FFI symbol `WinUsb_ResetDevice` that fails at link time.  
Evidence: `cargo test` / `cargo build --examples` fails with `undefined reference to WinUsb_ResetDevice` at `windows.rs:850`.

3.
Location: `usb-lib/src/backend/windows.rs:1116-1131`, `1137`, `1165-1179`, `1185`  
Severity: P0  
Finding: overlapped timeout path can leave I/O in flight while dropping `OVERLAPPED` + event + caller buffer references.  
Evidence: on timeout branch, code returns `UsbError::Timeout` without cancellation (`CancelIoEx` absent), then closes event and returns.

4.
Location: `usb-lib/src/hotplug.rs:120-123`, `131-137`  
Severity: P0  
Finding: Windows hotplug callback reads union member `DeviceInterface.SymbolicLink` before validating action type, risking invalid union-field access.  
Evidence: symbolic-link pointer is dereferenced before checking `action == DEVICEINTERFACEARRIVAL/REMOVAL`.

5.
Location: repository-wide test surface (`rg` found no `#[test]` in `usb-lib/src` or `usb-lib/examples`)  
Severity: P0  
Finding: no automated tests exist for API/backends/parsers.  
Evidence: `cargo test --lib` reports `running 0 tests`; no `usb-lib/tests` directory exists.

### P1

6.
Location: `usb-lib/src/backend/macos.rs:343-472` and `usb-lib/src/backend/mod.rs:59-253`  
Severity: P1  
Finding: macOS only implements 6/26 `UsbDevice` trait methods; the rest inherit `Unsupported` defaults.  
Evidence: macOS impl contains only descriptors + claim/release + control; no overrides for bulk/interrupt/pipe/device/async/iso/BOS/hub methods.

7.
Location: `usb-lib/src/backend/macos.rs:397-423`  
Severity: P1  
Finding: macOS interface claim/release is logical bookkeeping, not real IOUSBInterface open/close claim semantics.  
Evidence: `claim_interface` validates config + GET_INTERFACE and inserts into `HashSet`; `release_interface` calls SET_INTERFACE alt 0 + removes from set.

8.
Location: `usb-lib/src/hotplug.rs:269-282`, `README.md:14`, `25`  
Severity: P1  
Finding: README claims macOS hotplug support, but macOS hotplug path returns `Unsupported`.  
Evidence: non-Windows/non-Linux `platform::register` returns `Err(UsbError::Unsupported)`.

9.
Location: `usb-lib/src/backend/linux.rs:834-845`  
Severity: P1  
Finding: Linux pipe policy APIs are explicit stubs (`Unsupported`).  
Evidence: both `get_pipe_policy` and `set_pipe_policy` unconditionally return `Err(UsbError::Unsupported)`.

10.
Location: `usb-lib/src/backend/linux.rs:848-883`  
Severity: P1  
Finding: Linux async APIs are synchronous wrappers, not true async submission/completion paths.  
Evidence: `async_*` methods directly call `bulk_*`/`interrupt_*`.

11.
Location: `usb-lib/src/backend/linux.rs` (no BOS/Hub overrides), `usb-lib/src/backend/mod.rs:169-177`  
Severity: P1  
Finding: Linux backend lacks BOS and Hub descriptor support.  
Evidence: `read_bos_descriptor` and `read_hub_descriptor` are not implemented in Linux impl, so defaults return `Unsupported`.

12.
Location: `README.md:50-52`, `71`, `74`, `77`, `87`; `usb-lib/src/api/context.rs:16`, `23`, `28`; `usb-lib/src/api/device_handle.rs:40`; `usb-lib/src/core/transfer.rs:16-100`  
Severity: P1  
Finding: public API docs are materially out of sync with code.  
Evidence: README uses `UsbContext::new()?`, `ctx.list_devices()`, `ctx.open_device(...)`, `ControlSetup::new(...)`, `control_transfer_in(...)`; code exposes `UsbContext::new()`, `devices()`, `open(...)`, no `ControlSetup::new`, and `control_transfer(...)`.

13.
Location: `usb-lib/src/api/async_transfers.rs:5-6`, `20`, `36`, `55`, `71`, `85`, `100`  
Severity: P1  
Finding: async wrapper docs claim `spawn_blocking`, implementation uses `block_in_place`.  
Evidence: doc comments repeatedly reference `spawn_blocking`, code paths call `tokio::task::block_in_place`.

### P2

14.
Location: `usb-lib/src/backend/windows.rs:555-567`, `580-590`  
Severity: P2  
Finding: timeout-policy setup errors are silently swallowed.  
Evidence: `WinUsb_SetPipePolicy` results are assigned to `_` and ignored.

15.
Location: `usb-lib/src/backend/macos.rs:194`  
Severity: P2  
Finding: `MacOsDevice.path` appears unused (dead state).  
Evidence: field is set in `open` and not read in backend logic.

16.
Location: `README.md:105`, `109`  
Severity: P2  
Finding: README references missing repo files (`CONTRIBUTING.md`, `LICENSE`).  
Evidence: those files are absent in repository root during audit.

## Unsafe Block Review Summary

- Windows backend uses extensive unsafe FFI. Most blocks have local safety comments and basic null/handle checks.
- Confirmed high-risk unsafe issues:
  - Timeout path lifetime violation risk in overlapped I/O helpers (`windows.rs:1090-1187`).
  - Hotplug callback union member access before action discrimination (`hotplug.rs:120-123`).
- Linux unsafe usage is mostly confined to ioctl calls with typed structs; no immediate pointer-aliasing issue found in audited paths.
- macOS unsafe usage cannot be validated meaningfully for runtime safety until compile failures are fixed.

## API Layer Audit

- Public API in `lib.rs` and `api/*` does **not** match README usage examples.
- All `UsbDevice` trait methods are reachable from public API through `DeviceHandle` wrappers (`device_handle.rs:20-188`), but backend support varies by platform.
- Dead/unreachable concerns:
  - macOS helper methods for alt-setting exist (`macos.rs:290`, `301`) but trait methods `get_alternate_setting`/`set_alternate_setting` are not implemented, so public API cannot reach them.
  - `MacOsDevice.path` appears dead state (`macos.rs:194`).

## Test Audit

- Total tests: **0**
- Coverage: none (no parser tests, no backend contract tests, no API smoke tests, no integration tests)
- Completely untested areas:
  - descriptor parser correctness under malformed inputs
  - backend transfer semantics and timeout handling
  - hotplug callback safety/lifecycle
  - feature-gated paths (`tokio`, `isochronous`)
  - README example correctness against real API

## Platform Completion Percentages

Method-completion basis used: `implemented (non-default Unsupported) / 26 UsbDevice methods`.

- Windows: **100% source method coverage** (26/26), but blocked by P0 linker + unsafe issues.
- Linux: **85% source method coverage** (22/26 fully implemented; pipe policy unsupported + BOS/Hub missing + partial async/abort semantics).
- macOS: **23% source method coverage** (6/26 implemented), and target currently fails to compile.

## Ranked P0 Blockers Before Production Use

1. macOS backend compile failure on macOS target.
2. Windows `WinUsb_ResetDevice` unresolved link failure (tests/examples cannot link).
3. Windows overlapped timeout lifetime safety bug (potential in-flight I/O with dropped buffers/OVERLAPPED).
4. Windows hotplug callback union-field access before action validation.
5. No automated tests.

## Recommended Fix Sequence

1. Fix unsafe correctness first:
   - Add cancellation/cleanup for overlapped timeout paths.
   - Validate `action` before union-field dereference in hotplug callback.
2. Fix Windows link blocker:
   - Replace or correctly link `reset_device` path; prove with `cargo test` and `cargo build --examples`.
3. Make macOS compile:
   - Align FFI layer/dependencies with actual available `iokit_sys` symbols and correct CoreFoundation allocator types.
4. Implement missing macOS trait surface (bulk/interrupt, pipe/device controls, async, descriptors, hotplug).
5. Fill Linux remaining gaps (BOS/Hub, true async semantics if required).
6. Add tests and CI matrix (Windows/Linux/macOS) before claiming production readiness.
7. Rewrite README examples to match current public API exactly.
