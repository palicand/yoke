# yoke-volume — design

- **Date:** 2026-05-17
- **Status:** Approved, ready for implementation plan
- **Sub-project ID:** C (`yoke-volume`)

## Context

The QuadStick exposes a FAT mass-storage interface when its preferences allow.
A configuration is a set of `*.csv` files on that volume — `default.csv`
(always loaded), zero or more game-specific configs, and a singleton
`prefs.csv`. The previous sub-project (B, `yoke-config`) parses and writes the
bytes; this sub-project is everything *around* those bytes: discovering the
volume, knowing when it appears and disappears, listing what is on it, and
moving file contents on and off safely.

The discovery problem is non-trivial on Apple Silicon. The QuadStick's
mass-storage interface enumerates racily — the volume mount frequently
appears late, disappears on sleep/wake, and is hidden entirely when
`enable_DS3_emulation` is set to a value that does not expose mass storage
or when the device is in PS4/Hori emulation mode. The user-facing affordance
"plug your QuadStick in and edit a profile" requires the software to
distinguish "no device at all" from "device present, mass storage hidden"
from "everything is fine, here is the mount".

This sub-project produces two crates: `yoke-volume` (host-only, platform-
neutral core — trait, types, errors, a test backend) and `yoke-volume-macos`
(darwin-only DiskArbitration + IOKit backend). A future `yoke-volume-windows`
will follow when sub-project H lands. The trait split is deliberate — every
downstream consumer (`yokectl`, `yoke-tauri`) depends on `yoke-volume` for
the contract and picks a backend crate per `cfg(target_os)`.

## Goals

1. A `VolumeProvider` trait that abstracts mount discovery, listing,
   reading, writing, deleting, and renaming profiles. One contract, multiple
   backends.
2. A real macOS backend that uses DiskArbitration for mount events and IOKit
   for USB-device enumeration, so the UI can distinguish the three meaningful
   states (no device / device-but-no-volume / volume-present).
3. A test backend (`FsBackend`) compiled on every host that points at a
   local directory and faithfully implements the trait against the
   filesystem. Sufficient for `yoke-volume` unit tests, `yokectl` smoke
   flows, end-to-end integration tests, and the eventual
   `yokectl --fake-volume <dir>` agent affordance.
4. Write safety on a removable medium. A user can yank the cable at any
   moment; partially-written files must not be observable to the firmware.
5. Survives mount races and sleep/wake. State updates flow through tokio
   channels so consumers can react instead of polling.

## Non-goals

- **No CSV parsing or vocabulary knowledge.** This crate moves bytes. Parsing
  is `yoke-config`'s job; the `Profile` type is not imported by `yoke-volume`
  production code. (Test code is permitted to depend on `yoke-config` as a
  dev-dependency for end-to-end round-trip integration tests — see § 9.)
- **No HID, no serial, no device commands.** Those are `yoke-device`
  (sub-project G).
- **No live device push.** Saves go to the mounted FAT volume only. Stage I
  replaces this with the HID command channel once `yoke-device` reaches
  parity; the trait stays.
- **No firmware flashing in this sub-project.** Firmware flashing is a
  far-future stage (J) and lives behind its own crate / sub-project once
  the protocol notes mature and the bricking-risk gates are in place. It
  does not touch `yoke-volume`: flashing is a device-channel operation,
  not a volume operation. See § Out of scope.
- **No multi-device support in v1.** A user with two QuadSticks plugged in
  sees the first one and a `MultipleDevicesDetected` warning event. The
  trait can be extended to multi-device later, but that breaks every
  consumer signature so it is a separate sub-project.
- **No CLI, no UI.** `yokectl` (D) and `yoke-tauri` (E) are the consumers.
- **No Linux backend.** A placeholder may eventually live in
  `yoke-volume-linux`; the scaffold spec keeps the flake's Linux block
  commented out. Not in scope for this sub-project.
- **No WASM target.** This crate is host-only by design; backend code touches
  IOKit, DiskArbitration, and `std::fs`.
- **No persistence of last-known-mount or device history.** Statelessness is
  a feature; restarting the app means re-running discovery.

## Design

### 1. Workspace layout: two crates

```text
yoke/crates/
├── yoke-volume/                 # host-only, platform-neutral core
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs               # re-exports the public surface
│   │   ├── provider.rs          # trait VolumeProvider
│   │   ├── state.rs             # MountState, MountEvent, VidPid, ModeHint,
│   │   │                        # QUADSTICK_VID_PIDS const
│   │   ├── profile.rs           # ProfileName, ProfileEntry, ProfileKind
│   │   ├── error.rs             # VolumeError
│   │   └── fs_backend.rs        # FsBackend implementation
│   └── tests/
│       └── integration.rs       # end-to-end tests with yoke-config dev-dep
└── yoke-volume-macos/           # darwin-only DiskArbitration backend
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── provider.rs          # MacOsVolumeProvider
        ├── disk_arbitration.rs  # DA session + callbacks (minimal FFI)
        ├── iokit_usb.rs         # IOUSBDevice enumeration & notifications
        └── run_loop.rs          # dedicated CFRunLoop thread + lifecycle
    └── examples/
        └── watch.rs             # 30-line smoke binary
```

`yoke-volume` deps (added via `cargo add`):

- `tokio` with features `["sync", "macros"]` only — we use `sync::watch`,
  `sync::broadcast`, nothing else.
- `thiserror` — workspace rule.
- `tracing` — workspace rule.
- `serde` with `["derive"]` — `MountState` and `MountEvent` cross the Tauri
  IPC boundary in sub-project E; serde derives now means no DTOs later.

`yoke-volume` `[dev-dependencies]`:

- `yoke-config` (workspace path dep) — for end-to-end model-level round-trip
  integration tests. Production code does not import `yoke-config`.
- `tempfile` — for tempdir creation in `FsBackend` tests.
- `tokio` with the `rt` and `time` features additionally enabled — the
  integration tests need a current-thread runtime to drive the event-stream
  receivers.

`yoke-volume-macos` deps:

- `yoke-volume` (workspace path dep).
- `core-foundation` — Rust-friendly CF wrappers.
- `core-foundation-sys` — raw CF types when the safe wrappers do not cover
  what we need (mostly DA-side dictionary peeking).
- `libc` — `c_void`, integer typedefs for the FFI signatures.
- `tracing` — workspace rule.

The FFI surface is hand-rolled. Six DiskArbitration symbols
(`DASessionCreate`, `DASessionScheduleWithRunLoop`,
`DARegisterDiskAppearedCallback`, `DARegisterDiskDisappearedCallback`,
`DARegisterDiskDescriptionChangedCallback`, `DADiskCopyDescription`,
`DADiskCopyIOMedia`) and five IOKit-USB symbols
(`IOServiceGetMatchingServices`, `IOServiceMatching`,
`IOServiceAddMatchingNotification`, `IORegistryEntryGetParentEntry`,
`IORegistryEntryCreateCFProperty`) live as `extern "C"` blocks in
`disk_arbitration.rs` / `iokit_usb.rs`. The published `disk-arbitration-sys`
and `io-kit-sys` crates exist but bring in more surface than we need and
both have stale maintenance signals; the local FFI is small enough to own.

Workspace `members` grows from `["crates/yoke-config"]` to
`["crates/yoke-config", "crates/yoke-volume", "crates/yoke-volume-macos"]`.
`yoke-volume-macos` declares its `core-foundation` / `core-foundation-sys`
/ `libc` deps under `[target.'cfg(target_os = "macos")'.dependencies]`;
all crate code is `#[cfg(target_os = "macos")] mod ...` gated, so on
Linux and Windows the crate is an empty shell. This keeps
`cargo check --workspace` clean on every host that CI cares about, and
makes the platform split obvious at the Cargo dependency layer.

### 2. The `VolumeProvider` trait

```rust
// yoke-volume::provider

pub trait VolumeProvider: Send + Sync + 'static {
    fn current_state(&self) -> MountState;
    fn subscribe_state(&self) -> tokio::sync::watch::Receiver<MountState>;
    fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<MountEvent>;

    fn list_profiles(&self) -> Result<Vec<ProfileEntry>, VolumeError>;
    fn read_profile(&self, name: &ProfileName) -> Result<Vec<u8>, VolumeError>;
    fn write_profile(&self, name: &ProfileName, bytes: &[u8]) -> Result<(), VolumeError>;
    fn delete_profile(&self, name: &ProfileName) -> Result<(), VolumeError>;
    fn rename_profile(&self, from: &ProfileName, to: &ProfileName) -> Result<(), VolumeError>;
}
```

Semantics:

- `current_state()` is a cheap snapshot read; it never blocks on I/O.
- `subscribe_state()` returns a tokio `watch::Receiver` whose latest value
  is always the same as `current_state()`. Use it when you want a continuous
  "current value" subscription.
- `subscribe_events()` returns a tokio `broadcast::Receiver` carrying every
  transition. Use it when you want every event (mount, unmount,
  device-appeared, etc.), not just the latest state. The broadcast channel
  is bounded at 64; slow subscribers may see `RecvError::Lagged`.
- All I/O methods return `Err(VolumeError::NotPresent)` when state is
  `Absent`, and `Err(VolumeError::VolumeHidden { hint })` when state is
  `DeviceVisibleNoVolume`. The trait does not offer an "ensure mounted"
  helper — callers either subscribe and wait, or accept the synchronous
  failure and surface it.
- Send + Sync + 'static so the trait object can live in an `Arc<dyn
  VolumeProvider>` shared across tokio tasks.

### 3. State and event types

```rust
// yoke-volume::state

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MountState {
    Absent,
    DeviceVisibleNoVolume { vid_pid: VidPid, mode_hint: Option<ModeHint> },
    Present { mount_point: PathBuf, vid_pid: VidPid, label: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MountEvent {
    DeviceAppeared { vid_pid: VidPid },
    DeviceDisappeared,
    DeviceModeChanged { vid_pid: VidPid, mode_hint: Option<ModeHint> },
    VolumeMounted { mount_point: PathBuf, vid_pid: VidPid, label: String },
    VolumeUnmounted,
    MultipleDevicesDetected { count: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct VidPid { pub vendor: u16, pub product: u16 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModeHint {
    Ps4OrHori,            // Hori VID/PID seen (0x0F0D / 0x0066)
    MassStorageDisabled,  // QuadStick VID/PID seen but no QuadStick FAT mount
    Emulation,            // Unknown VID/PID at the port where we last saw a
                          // confirmed Quad Stick — a profile activated a
                          // third-party persona (Sony DS3, Xbox, Switch,
                          // generic gamepad, ...). The persona's VID/PID is
                          // carried in MountState::DeviceVisibleNoVolume.
}

pub const QUADSTICK_VID_PIDS: &[VidPid] = &[
    VidPid { vendor: 0x16D0, product: 0x092B },  // primary
    VidPid { vendor: 0x16D0, product: 0x092C },  // X360CE emulation
    VidPid { vendor: 0x16D0, product: 0x092D },  // PS4/Switch emulation alias
    VidPid { vendor: 0x16D0, product: 0x092E },  // alias
    VidPid { vendor: 0x1FC9, product: 0x205B },  // legacy unit
];
pub const HORI_PS4_VID_PID: VidPid = VidPid { vendor: 0x0F0D, product: 0x0066 };
```

Both backends share the same VID/PID catalog. The Hori fallback is broken
out because seeing it is the signal for `ModeHint::Ps4OrHori`.

`QUADSTICK_VID_PIDS` is intentionally **not** exhaustive of every persona
the device can adopt. QuadStick profiles can flip the device into
third-party emulation (Sony DualShock 3, Xbox 360, Switch Pro, generic
gamepad, ...), each of which re-enumerates under the impersonated
vendor's VID:PID rather than `0x16D0`. The macOS backend recognizes these
via physical-port anchoring (§ 6) instead of a per-persona catalogue, so
this const can stay small and stable.

`PathBuf` serializes as a string. On macOS this is always something under
`/Volumes/`; the field is a `PathBuf` for type honesty, not for
cross-platform handling.

### 4. ProfileName, listing, kind

```rust
// yoke-volume::profile

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ProfileName(String);  // private invariant: validated, with `.csv`

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProfileKind { Default, Prefs, Game }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ProfileEntry {
    pub name: ProfileName,
    pub kind: ProfileKind,
    pub byte_len: u64,
    pub modified: std::time::SystemTime,
}
```

`ProfileName::new(raw: &str)` validation:

- Strip a trailing `.csv` if present.
- Reject if the stem is empty.
- Reject if the stem contains any of `/`, `\`, `\0`, or any non-printable.
- Reject if the stem contains `:`, `<`, `>`, `|`, `?`, `*`, `"` (FAT-illegal).
- Reject if the stem is longer than 64 chars.
- Re-attach `.csv` and store.

`ProfileName::kind` maps `default.csv` → `ProfileKind::Default`, `prefs.csv` →
`ProfileKind::Prefs`, everything else → `ProfileKind::Game`. Case-insensitive
match on the special two — FAT is case-insensitive on macOS and Windows.

`list_profiles` semantics:

- Read `mount_point` non-recursively. The QuadStick firmware reads from the
  volume root only; subdirectories are user clutter.
- Include every entry with `.csv` extension, case-insensitive.
- Exclude hidden files (`.DS_Store`, leading-dot anything, `__MACOSX/`,
  `.Spotlight-V100/` etc.).
- Sweep stale `.tmp.*` siblings older than 60 s: log a `tracing::warn!` and
  `fs::remove_file`. A `.tmp.*` younger than 60 s is left alone (a concurrent
  write may be in progress).
- Sort by name for deterministic output.

### 5. Write atomicity

```text
write_profile(name, bytes):
    let final = mount/<name>.csv
    let tmp   = mount/<name>.csv.tmp.<rand_u32>
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;       // best-effort fsync; FAT may no-op the data sync
    }
    fs::rename(&tmp, &final)?;   // atomic from observability perspective
```

On any `Err` after `File::create` and before `rename`, a best-effort
`fs::remove_file(&tmp)` runs in a guard. The `.tmp.*` random suffix
makes concurrent writes safe (two callers in the same process; we do
not coordinate cross-process). `sync_all` is correctness on case the
device is yanked; FAT may not honor it but the call is free when it
does no harm.

We do not `fsync` the directory entry — FAT does not journal directory
metadata and the macOS `mount_msdos` driver does not expose a useful
flush at that level. The rename is observed atomically by any reader
because POSIX `rename(2)` on the same filesystem is atomic; the
firmware reads only after the user yanks the cable, by which point the
buffer cache is in whatever state it is in.

### 6. macOS backend mechanics

`MacOsVolumeProvider::new() -> Result<Self, VolumeError>` does:

1. Create the tokio channels (`watch::channel(MountState::Absent)` and
   `broadcast::channel(64)`) on the calling thread.
2. Spawn a dedicated OS thread named `yoke-volume-da`. Stack frame on
   that thread:
   - `CFRunLoopGetCurrent()`.
   - `DASessionCreate(kCFAllocatorDefault)`, `DASessionScheduleWithRunLoop`
     on the current run loop in mode `kCFRunLoopDefaultMode`.
   - Register `DARegisterDiskAppearedCallback`,
     `DARegisterDiskDisappearedCallback`,
     `DARegisterDiskDescriptionChangedCallback`. The user context pointer
     is an `Arc<Inner>` cloned into the FFI.
   - Create `IONotificationPortRef`,
     `IONotificationPortGetRunLoopSource`, add to current run loop.
   - `IOServiceAddMatchingNotification` for `kIOMatchedNotification` and
     `kIOTerminatedNotification`, matching `IOUSBDevice`. A single
     registration is used; the callback reads `idVendor` / `idProduct`
     and filters against `QUADSTICK_VID_PIDS` / `HORI_PS4_VID_PID`. This
     avoids ordering coupling between multiple VID-specific registrations.
   - Drain currently-attached USB devices via
     `IOServiceGetMatchingServices` with the same matching dictionary.
     Drain currently-mounted disks by iterating `/Volumes/` and calling
     `DADiskCreateFromVolumePath` on each candidate (DA's appearance
     callback does *not* fire for disks mounted before registration, so
     this enumeration is required for first-state seeding).
   - Seed `MountState` from drained data; `state_tx.send_replace(...)`
     and emit a single initial event.
   - `CFRunLoopRun()`. Returns only when `CFRunLoopStop` is called on
     drop.
3. The constructor returns once the seeding is complete. We use a
   `std::sync::mpsc::sync_channel(0)` rendezvous between the calling
   thread and the DA thread for the "seeding complete" handshake.

Internal state struct:

```rust
struct Inner {
    state: Mutex<MountState>,
    state_tx: watch::Sender<MountState>,
    event_tx: broadcast::Sender<MountEvent>,
    devices_seen: Mutex<HashSet<DeviceKey>>,   // for MultipleDevicesDetected
}
```

Where `DeviceKey` is whatever uniquely identifies a connected USB device
(IOKit `IOService` registry id, fetched via
`IORegistryEntryGetRegistryEntryID`).

On every callback (DA disk appeared / disappeared / description changed /
IOKit device matched / IOKit device terminated):

1. Resolve the affected disk to its parent `IOMedia`, then to its
   grandparent `IOUSBDevice`, via
   `IORegistryEntryGetParentEntry` walks. Read its
   `idVendor` / `idProduct` properties. Compare against
   `QUADSTICK_VID_PIDS` and `HORI_PS4_VID_PID`.
2. Recompute the desired `MountState` from the union of (USB devices
   currently present) and (DA disks currently mounted with QuadStick
   ancestry).
3. `state_tx.send_replace(new_state.clone())`; emit the corresponding
   `MountEvent`(s) on the broadcast channel. Lagged subscribers see
   `RecvError::Lagged(n)`; they can call `current_state()` to resync.
4. If two QuadStick devices are simultaneously present, emit
   `MultipleDevicesDetected { count }` and pick the lowest registry-id
   one as "the device".

`Drop` for `MacOsVolumeProvider` posts a `CFRunLoopStop` to the DA
thread's run loop, joins the thread, releases CF objects. All FFI
pointer ownership is tracked via `CFRetain` / `CFRelease`; ownership
of `IOServiceObject_t` via `IOObjectRelease`. Run-loop stop is
robust against the rare race where a callback is firing during
`Drop`: we hold `Arc<Inner>` through the FFI registration, so the
callback can complete safely; the thread join waits for the run
loop to drain.

**LocationID anchoring for emulation profiles.** During USB drain the
watcher reads each device's IOKit `locationID` alongside
`idVendor` / `idProduct`. When it sees a confirmed Quad Stick
(VID/PID in `QUADSTICK_VID_PIDS`) it remembers that `locationID` in
session-scoped state. On subsequent drains, any device classified as
`Other` whose `locationID` matches that anchor is treated as the
QuadStick in some emulation persona and surfaces as
`MountState::DeviceVisibleNoVolume { vid_pid: <persona>, mode_hint:
Some(Emulation) }`. The persona's `vid_pid` is propagated through to
state and into the `DeviceModeChanged` event so the UI can describe
*what* is being emulated without us having to maintain an exhaustive
mapping. This is what lets the backend keep recognizing the device
across DualShock 3, Xbox, Switch, and other third-party emulation
profiles whose VID/PIDs we have not enumerated.

The anchor is **session-scoped and one-way sticky**: it is set when a
confirmed Quad Stick is seen, never cleared. The trade-off is a
cold-start gap — if the watcher launches while the device is already
in an emulation persona it has no anchor yet and reports `Absent`
until the user flips back to base mode once. That fail-safe is
deliberate: without a confirmed Quad Stick sighting at that port we
cannot tell the device apart from a real third-party controller
plugged into the same hub.

The trait methods on `MacOsVolumeProvider` read the snapshot mutex
for `current_state` and clone-out a `watch::Receiver` /
`broadcast::Receiver` for `subscribe_state` / `subscribe_events`.
I/O methods (`list_profiles`, `read_profile`, etc.) check
`current_state` first and return `NotPresent` / `VolumeHidden`
when applicable; otherwise they execute against the
`mount_point` from the snapshot.

### 7. FsBackend (test impl, lives in `yoke-volume`)

```rust
pub struct FsBackend {
    root: PathBuf,
    inner: Arc<FsInner>,
}

struct FsInner {
    state: Mutex<MountState>,
    state_tx: watch::Sender<MountState>,
    event_tx: broadcast::Sender<MountEvent>,
}

impl FsBackend {
    pub fn new(root: PathBuf) -> Self { ... }
    pub fn set_present(&self, present: bool) { ... }   // test-only state nudge
}
```

`new(root)` publishes `Present { mount_point: root.clone(), vid_pid:
VidPid { 0, 0 }, label: "fs-backend".into() }` if `root.is_dir()`,
else `Absent`. No DiskArbitration. No background thread.

`set_present(false)` flips the state to `Absent` and emits
`MountEvent::VolumeUnmounted`. `set_present(true)` flips back to
`Present`. This is the test affordance for "simulate unplug".

I/O methods are straightforward `std::fs` operations on `root`:

- `list_profiles`: directory walk on `root` with the same hidden-file
  and `.tmp.*` sweep rules as the macOS backend.
- `read_profile(name)`: `fs::read(root.join(name.as_filename()))`.
- `write_profile(name, bytes)`: same write-to-`.tmp` + rename as the
  macOS backend.
- `delete_profile`, `rename_profile`: direct `fs::remove_file` /
  `fs::rename`.

Used by:

- `yoke-volume` unit tests (tempdir + assertions).
- `yoke-volume` end-to-end integration tests (see § 9).
- `yokectl` integration tests (sub-project D).
- `yokectl --fake-volume <path>` flag, when sub-project D lands.
- The "agent flow that has no real device" affordance.

### 8. Errors and warnings

```rust
#[derive(thiserror::Error, Debug)]
pub enum VolumeError {
    #[error("no QuadStick volume mounted")]
    NotPresent,
    #[error("device visible but volume hidden: {hint:?}")]
    VolumeHidden { hint: Option<ModeHint> },
    #[error("invalid profile name: {0}")]
    InvalidProfileName(String),
    #[error("backend init failed: {0}")]
    BackendInit(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

`NotPresent` and `VolumeHidden` are *expected* failures — the UI shows
them as state, not as errors. `BackendInit` only fires from
`MacOsVolumeProvider::new` when DA session create or IOKit notification
port create fails.

Non-fatal observations go through `tracing` (`warn!` for stale `.tmp`
sweeps, `info!` for state transitions, `debug!` for callback details).
Nothing is silent.

### 9. Tests

#### 9.1 `yoke-volume` unit tests

All using `tempfile::tempdir`:

- `ProfileName::new` validation table — accept/reject cases, the `.csv`
  suffix handling, kind classification.
- `FsBackend` round-trip of all six trait methods.
- `FsBackend::set_present(false)` flips state to `Absent`, emits
  `VolumeUnmounted`, and subsequent I/O returns `NotPresent`.
- Stale `.tmp.*` sweep: pre-create a `.tmp.<rand>` file with mtime
  61 s ago, call `list_profiles`, assert it is gone and a warn was
  logged.
- Write atomicity: write_profile injecting a forced error after
  `File::create` leaves no orphan files.

#### 9.2 `yoke-volume` end-to-end integration tests

Live in `crates/yoke-volume/tests/integration.rs`. `yoke-config` is a
dev-dependency (production code in `yoke-volume` does not import it; tests
do, deliberately, to exercise the full "edit a profile, save it, read it
back" path the UI will later use).

Required test cases:

1. **Byte-level round-trip via `FsBackend`.** Construct an `FsBackend` on
   a `tempdir`. Take a fixture CSV string (lifted from `yoke-config`'s
   round-trip test fixtures so we know it is a real, parseable
   QuadStick profile). Call `write_profile(name, bytes)`, then
   `read_profile(name)`, and assert the returned bytes are identical to
   the input bytes. Verifies that `write_profile`'s tmp + rename
   doesn't mangle data and that `read_profile` reads the same file
   `write_profile` wrote.

2. **Model-level round-trip via `FsBackend` + `yoke-config`.** Same
   fixture CSV. Parse with `yoke_config::parse(bytes)` to get a
   `Profile`. Call `write_profile(name, bytes)`. Call
   `read_profile(name)`. Parse the result with `yoke_config::parse`.
   Assert the two `Profile` models compare equal. This is the smoke
   test the UI's save/load round-trip becomes in sub-project F; if
   this passes, the UI plumbing is correct in principle.

3. **Multi-profile lifecycle.** Write three profiles (`default`,
   `destiny`, `forza5`) with different fixture CSVs. Call
   `list_profiles` and assert all three are present with the right
   `ProfileKind` classification (default → `Default`, others → `Game`).
   Call `rename_profile("destiny", "destiny2")`; list again and assert
   the renaming took effect. Call `delete_profile("forza5")`; list
   again and assert only two profiles remain. `read_profile` on the
   deleted name returns `Io(...)` with `ErrorKind::NotFound`.

4. **`NotPresent` failure path.** Construct an `FsBackend`. Call
   `set_present(false)`. Assert every I/O method returns
   `Err(VolumeError::NotPresent)`. Call `set_present(true)`. Assert
   the operations succeed again.

5. **Event-stream observation.** Construct an `FsBackend` inside a
   `tokio::runtime::Builder::new_current_thread().enable_time().build()`
   runtime. Subscribe to `subscribe_events`. Call `set_present(false)`
   and `set_present(true)`. Assert the corresponding `VolumeUnmounted`
   / `VolumeMounted` events arrive within a 100 ms timeout. Verifies
   the tokio channel plumbing actually publishes.

These tests run on every host that builds the workspace; they do not
need a real device and they do not depend on platform-specific
backends. They are the load-bearing acceptance test that proves the
trait + FsBackend + yoke-config pipeline is correct end-to-end before
any UI work begins.

#### 9.3 `yoke-volume-macos` tests

- A `#[test]` that compiles only on darwin and constructs
  `MacOsVolumeProvider::new()` then drops it. Verifies the DA session
  opens, the run loop thread starts and joins cleanly. No assertion
  about device presence.
- A `#[test]` that early-returns when `std::env::var("YOKE_REAL_DEVICE")`
  is not `"1"`. When the env var is set, it constructs the provider,
  subscribes, and asserts a sensible `MountState` within 3 s. CI does not
  set the env var so the test is a no-op on the darwin runner.
- A `cargo run --example watch -p yoke-volume-macos` smoke binary
  (30 lines) that prints state transitions. The maintainer runs this
  once against a real QuadStick — sip-and-puff modes, mass-storage
  toggles, sleep/wake — to confirm the state transitions are sane.
  Not part of CI; the example exists so future maintainers have a
  three-line repro for backend regressions.

### 10. CI

The existing `yoke/.github/workflows/ci.yml` continues to gate on
`hashFiles('crates/**/Cargo.toml') != ''`, which already became true
in sub-project B. No CI changes needed; the two new crates compile
on darwin and Linux runners alike (the macOS-only crate is an empty
shell off-darwin).

Regression guards retained from prior sub-projects:

- `cargo build -p yoke-config --target wasm32-unknown-unknown` still
  passes — `yoke-volume` and `yoke-volume-macos` are not added to
  yoke-config's dependency graph, so the WASM build is unaffected.
- `cargo metadata --no-deps` still parses the workspace cleanly.

### 11. Acceptance criteria

This sub-project is done when:

- `crates/yoke-volume/` and `crates/yoke-volume-macos/` both exist
  with the layout in § 1.
- The workspace `members` array includes both.
- `cargo build --workspace` is clean on macOS and Linux.
- `cargo clippy --workspace --all-targets -- -D warnings` is clean
  on macOS and Linux.
- `cargo test --workspace` is clean on macOS and Linux. (On Linux,
  the macOS-only tests in `yoke-volume-macos` are cfg'd out.)
- The five end-to-end integration tests in § 9.2 all pass.
- `cargo build -p yoke-config --target wasm32-unknown-unknown` still
  passes.
- The maintainer has run `cargo run --example watch -p
  yoke-volume-macos` against a real QuadStick once and confirmed the
  observed transitions: plug-in → `DeviceAppeared` then
  `VolumeMounted`; switch device to PS4 mode → `VolumeUnmounted` then
  state becomes `DeviceVisibleNoVolume { hint: Some(Ps4OrHori) }`;
  switch device to a non-Hori emulation profile (DS3 / Xbox / ...) →
  state becomes `DeviceVisibleNoVolume { hint: Some(Emulation) }` via
  locationID anchoring (no spurious `DeviceDisappeared`); switch back
  to base mode → `VolumeMounted`; unplug → `DeviceDisappeared`, state
  `Absent`. The example does not ship CI assertions for these — it is
  a maintainer-validated smoke test, recorded in the PR description.

## Out of scope (queued for future sub-projects)

- **D — `yokectl`:** CLI surface built on `yoke-config` + `yoke-volume`.
  Adds the `--fake-volume <path>` flag that wires through to
  `FsBackend`.
- **E — Tauri shell + UI v1 (read-only viewer):** `yoke-tauri`
  subscribes to `MountState` and `MountEvent`, bridges them to the
  Leptos frontend via Tauri IPC. The serialization story is already
  paid for by serde derives in this crate.
- **F — UI v2 (editor):** consumes `write_profile` /
  `delete_profile` / `rename_profile`.
- **G — `yoke-device`:** HID 0xFF00 + serial transports. May share
  IOKit-USB enumeration code with this crate; if so, the shared bits
  migrate to a `yoke-iokit` internal crate then.
- **H — Windows port:** `yoke-volume-windows` lands. Trait stays.
- **I — Live device push:** replaces volume saves with HID commands.
  `write_profile` stays available for users who prefer the
  mass-storage path.
- **J — Firmware flashing.** Lives in its own future sub-project,
  behind a `yoke-firmware` crate (likely a thin wrapper over
  `yoke-device`'s command channel plus a bootloader-mode entry
  sequence). Gated on `yoke-device` reaching `confirmed (…)` on
  every relevant protocol fact, plus explicit user-confirmation
  flows ("are you sure" prompts, firmware-hash verification,
  rollback path). Bricking risk is the entire reason this is
  staged so late. `yoke-volume` is not touched — flashing
  travels over the device channel, not the mass-storage volume.

## Forward references

- `yoke-config` (sub-project B) types are *not* imported by
  `yoke-volume` production code. Profiles round-trip as opaque
  `Vec<u8>`. Tests are allowed to depend on `yoke-config` (see § 9.2)
  to exercise the full edit-save-reload path the UI will need; this
  is a dev-dependency only.
- The QuadStick wire-protocol notes (in the maintainer's local
  Obsidian vault) document the VID/PID set; this crate is the first
  on-disk consumer of that catalog and pins it in
  `QUADSTICK_VID_PIDS`. Future updates to the catalog land in both
  places.
- The macOS DiskArbitration FFI here is a candidate for extraction
  to a shared `yoke-iokit` crate when sub-project G (`yoke-device`)
  needs IOKit-USB enumeration of its own. No premature extraction —
  do it then if and only if there is duplication worth eliminating.
