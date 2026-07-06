# yoke-volume-windows — Windows volume + device-state backend

**Date:** 2026-07-05
**Stage:** H
**Status:** proposed
**Predecessor:** [`2026-05-17-yoke-volume-design.md`](2026-05-17-yoke-volume-design.md)

## Goal

Full backend parity with `yoke-volume-macos` on Windows: a new
`yoke-volume-windows` crate implementing the `VolumeProvider` trait with
volume mount/unmount detection, USB device enumeration for mode hints
(`MassStorageDisabled`, `Emulation`, `Ps4OrHori`), and the `Mounting`
transient state — plus wiring so `yokectl` and `yoke-gui` run on Windows
with the native backend.

Because the developer's QuadStick is also their input device, capturing it
into a Windows VM removes their ability to control the host, and UTM's QEMU
passthrough does not make the composite MSC+HID device usable in the guest
at all. The design therefore treats "real QuadStick visible to Windows" as
a rare, final smoke test and makes everything else verifiable without it:
host-side unit tests over pure logic, a Windows CI leg, and live VM testing
against a plain USB stick impersonating a QuadStick via a test-only VID:PID
override.

## Non-goals

- Windows packaging, installer, code signing, or a release pipeline
  (separate sub-project).
- Linux support (the flake's commented-out Linux block stays commented).
- `yoke-device` (Stage G) and live device push (Stage I).
- Label-based volume detection. QMP-4 finds the drive by scanning letters
  for the volume label `"quad stick"` (`qsflash.py`); that breaks on
  renamed volumes and cannot yield a `VidPid`. This backend correlates by
  USB identity instead.

## Architecture

New crate, a structural twin of `yoke-volume-macos`. The authoritative module
layout is `crates/yoke-volume-windows/src/`; in outline:

- `lib.rs` — module wiring: `ids` and `tracked` are public on every target;
  the FFI modules (`provider`, `message_window`, `device_notify`, `usb_enum`)
  and the `WindowsVolumeProvider` re-export are `cfg(windows)`-gated.
- `provider.rs` — `WindowsVolumeProvider` plus the rescan/publish event loop
  and its `SetTimer` poll (analogue of the macOS `run_loop.rs` worker).
- `tracked.rs` — `Tracked` + `compute()`, the `MountState` decision tree.
- `ids.rs` — PnP-ID / UTF-16 string parsing (VID:PID, multi-sz).
- `message_window.rs` — twin of `run_loop.rs`: a thread owning a message-only
  window + pump.
- `device_notify.rs` — `RegisterDeviceNotification` plumbing (USB + volume
  interfaces).
- `usb_enum.rs` — twin of `iokit_usb.rs`: SetupAPI/CfgMgr32 VID:PID
  enumeration and volume→USB-parent correlation.

- The FFI modules are `cfg(windows)`-gated, but `ids` and `tracked` are pure
  and compile (and unit-test) on every host, so the state machine keeps
  coverage off Windows too. The [`windows`][winrs] crate (the single new
  dependency) sits under `[target.'cfg(windows)'.dependencies]`;
  tokio/tracing/yoke-volume match the macOS crate.
- Threading model matches macOS: one dedicated OS-event thread.
  `MessageWindowThread` mirrors `RunLoopThread`'s contract — spawn with a
  worker, setup handshake over a rendezvous channel (construction fails
  loudly if setup fails), teardown on Drop with a join. State publishes
  through the same tokio `watch`/`broadcast` channels on a shared `Inner`.
- Profile I/O needs no new code. `MountState::Present.mount_point` is the
  drive root (e.g. `E:\`) and the shared `yoke_volume::io` helpers plus
  `require_present_at` work unchanged over `std::fs`.

## Detection machinery

### Event delivery

Message-only windows do not receive broadcast `WM_DEVICECHANGE` volume
messages (`DBT_DEVTYP_VOLUME` is broadcast to top-level windows only), but
[`RegisterDeviceNotification`][rdn] targeted at a message-only window does
deliver device-interface notifications. The worker therefore registers two
interface classes on the hidden window:

- [`GUID_DEVINTERFACE_USB_DEVICE`][usbguid] — USB arrivals/removals
  (personas, mode changes)
- [`GUID_DEVINTERFACE_VOLUME`][volguid] — volume arrivals/removals

### Rescan on notification

Every notification triggers a full rescan rather than incremental parsing
of the event payload — the same philosophy as the macOS 1 s poll. `Tracked`
is recomputed from a fresh snapshot, so a missed or reordered event
self-heals on the next one. If event storms during re-enumeration prove
noisy in live testing, a `SetTimer`-based coalesce (~200 ms) on the same
window is the escape valve; it is not built until needed.

Per rescan:

1. Enumerate USB device instances; parse VID:PID from hardware IDs
   (`USB\VID_16D0&PID_092B&...`). Pure string parsing, host-testable.
2. Enumerate volume interfaces; for each, walk [`CM_Get_Parent`][cmparent]
   up the devnode tree until an ancestor instance ID matches `USB\VID_...`,
   correlating volume to USB device by identity. Resolve the drive letter
   via [`GetVolumePathNamesForVolumeNameW`][gvpn] and the label via
   [`GetVolumeInformationW`][gvi].
3. Feed a `Tracked` twin (same fields as the macOS one minus BSD names,
   plus "volume devnode seen") and publish through the shared
   `state_transition_events`.

### State mapping

| Windows observation | `MountState` |
|---|---|
| QuadStick VID:PID + volume with readable drive letter | `Present { mount_point: E:\, label }` |
| QuadStick VID:PID + volume devnode, no drive letter yet | `Mounting` |
| QuadStick VID:PID, no volume devnode | `DeviceVisibleNoVolume { MassStorageDisabled }` |
| Hori VID:PID | `DeviceVisibleNoVolume { Ps4OrHori }` |
| Unlisted VID:PID at the sticky port | `DeviceVisibleNoVolume { Emulation }` |

The macOS impl's sticky `location_id` (recognizing the device after it
re-enumerates under an unlisted emulation persona) maps to the USB device's
[`DEVPKEY_Device_LocationPaths`][locpaths], a stable physical-port path
string.

### Error handling

- Window creation / notification registration failures →
  `VolumeError::BackendInit`; construction fails loudly, and both consumers
  already handle that path.
- Win32 errors during a rescan → log via `tracing`, keep the last good
  state.
- Profile I/O errors flow through the existing `VolumeError::Io`.
- Drop: `UnregisterDeviceNotification`, `DestroyWindow` via a posted
  message, join the thread — mirroring `RunLoopThread`.

## Test-only VID:PID override

A shared helper in `yoke-volume` reads the env var `YOKE_TEST_VIDPIDS`
(format `16d0:092b,abcd:1234`) once at provider construction and *extends*
the built-in `QUADSTICK_VID_PIDS` set. Both platform backends consult it,
so the stick flow is testable on macOS before the VM. Two guards, per the
refuse-on-ambiguity rule:

- `tracing::warn!` whenever the override is active, so it cannot silently
  leak into normal use.
- A malformed value fails provider construction with `BackendInit` rather
  than being silently ignored.

## Consumer wiring

- `yokectl/src/backend.rs`: a `#[cfg(target_os = "windows")]` arm
  constructing `WindowsVolumeProvider`; the `--fake-volume` path and the
  other-OS bail-out stay as they are.
- `yoke-gui/src/main.rs`: a matching cfg arm with the same
  fallback-to-`FsBackend` error handling the macOS arm has.

## Testing

1. **Host unit tests** (run anywhere, including existing CI): hardware-ID
   parsing, the `Tracked::compute` mapping table above, override parsing.
   Transition-event logic is already covered in `yoke-volume`.
2. **Windows CI leg**: a new `windows-latest` job — rustup toolchain from
   `rust-toolchain.toml` (no Nix on Windows), `cargo check`,
   `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`. Compiles the real Win32 paths on every push
   (x86_64).
3. **Live VM testing**: UTM Windows 11 ARM guest with a plain USB stick
   passed through (mass-storage-only devices pass through fine in QEMU;
   the QuadStick never leaves the host). `YOKE_TEST_VIDPIDS` set to the
   stick's IDs. Covers mount/unmount/`Mounting`, profile
   read/write/rename/delete, and GUI wiring. Two sticks with different
   VID:PIDs swapped on one physical port exercise the Emulation-persona
   stickiness. Build natively in the guest (`aarch64-pc-windows-msvc` via
   rustup).
4. **Real-device smoke test** (persona re-enumeration timing — the one
   thing sticks cannot fake): attempt VMware Fusion passthrough with USB
   2.0 enabled before first guest boot. The escape hatch while the device
   is captured is the device-side mode change, which re-enumerates and
   releases the capture. If Fusion fails like UTM did, the gap is recorded
   as residual risk and Windows support ships with a beta caveat until a
   Windows-native tester confirms.
5. **Gated integration test**: `tests/real_device.rs` in the new crate,
   mirroring the macOS one — env-gated, run manually inside the VM.

The VM setup and test procedure get written down in
`crates/yoke-volume-windows/TESTING.md`.

## Acceptance

- `cargo test --workspace` green on macOS and on the Windows CI job.
- `cargo clippy --workspace --all-targets -- -D warnings` green on both.
- In the UTM guest with a stick + override: `yokectl device` reports
  `Present`, profile round-trip works, unplugging reports `Absent`, and
  `yoke-gui` shows the library.
- Persona table verified with two sticks; real-device persona timing
  verified in Fusion or explicitly recorded as residual risk.

[winrs]: https://github.com/microsoft/windows-rs
[rdn]: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerdevicenotificationw
[usbguid]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/guid-devinterface-usb-device
[volguid]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/guid-devinterface-volume
[cmparent]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_get_parent
[gvpn]: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumepathnamesforvolumenamew
[gvi]: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw
[locpaths]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-locationpaths
