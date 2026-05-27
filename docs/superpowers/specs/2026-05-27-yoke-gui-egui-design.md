# Yoke GUI — read-only profile viewer (egui)

**Date:** 2026-05-27
**Stage:** E
**Status:** proposed
**Predecessors:** [`2026-05-16-yoke-scaffold-design.md`](2026-05-16-yoke-scaffold-design.md), [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-17-yoke-volume-design.md`](2026-05-17-yoke-volume-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)

## Goal

Land the Yoke desktop GUI as a **read-only profile viewer** built with egui. The user can open a profile from the mounted QuadStick volume, from a local CSV file, or from the community index, and see the design-handoff three-region layout (rail + sub-profile strip + device sketch + bindings panel). Clicking a station filters the bindings panel. Live volume presence drives a status indicator. No editing, no save, no action picker.

## Why egui

The GUI is native: `eframe` renders through `wgpu`/`glow` with **no embedded WebView in the shipped binary**. For an all-Rust desktop app that reads local CSV files and talks to a USB device, a fully native, self-contained stack is the right fit — the UI calls the library crates directly, in-process, with no browser engine to ship, no IPC boundary, and no serialization layer between the frontend and the data.

## Scope

### In v1

- Volume-first launch: poll `yoke-volume` for a mounted QuadStick; device profiles populate the library when present.
- File-open fallback: a native file picker opens any local CSV.
- Community profiles: list entries via `yoke-index`.
- Three-region editor layout, dark (Console) theme, Map sketch view.
- Single rail entry "Profiles" plus a live DEVICE status section.

### Cut from v1

No action picker, no key-capture, no save / edit, no tweaks panel, no rail items beyond Profiles + DEVICE, no live content refresh, no Diagram/Grid sketch variants, no Studio/Contrast themes, no Windows/Linux, no recent-files. `yoke-edit` is **not** a dependency — this stage reads, it does not write.

| Cut | Lands in |
|---|---|
| Action picker, key-capture, binding/modifier edits | F |
| Save / Save-to-QuadStick / Preview CSV | F |
| Tweaks panel (theme/font/sketch/picker switching) | F |
| Diagram and Grid sketch variants | F or later |
| Studio and Contrast themes | F or later |
| Windows / Linux platform support | H |
| Live refresh of profile contents on external change | F |

## Architecture

### Single crate: `yoke-gui`

One new crate — a single binary that *is* the app. Because egui runs natively in the host process, there is no host/frontend split and no IPC seam: the UI consumes `yoke-config` / `yoke-volume` types directly, with no serde wire format and no separate backend process.

`eframe` compiles the same source to two targets:

- **native** (`cargo run -p yoke-gui`): the shipped app. Real window, real device, filesystem, network.
- **wasm** (`trunk serve` inside `crates/yoke-gui`): a browser dev build serving the `MockDataSource`. This satisfies the AGENTS.md UI-substrate principle — an agent that cannot see a native window can iterate in a browser via `trunk serve` against the mock. The shipped native binary contains no WebView; the browser build is a development convenience only, never the production runtime.

### The `DataSource` seam

The UI depends on a `DataSource` trait, never on a concrete impl. Two impls share the surface:

- `NativeDataSource` — wraps `yoke-volume` (presence + profile read), `yoke-index` (community), and `std::fs` (local file). Host-only.
- `MockDataSource` — serves one committed fixture CSV, an in-memory community list, and a hardcoded "Present" volume state. Used by the wasm dev build and by host unit tests.

Intended surface (signatures pinned at implementation; this captures intent):

```rust
trait DataSource: Send + Sync {
    fn volume_state(&self) -> VolumePresence;
    fn list_device_profiles(&self) -> Result<Vec<ProfileEntry>, DataError>;
    fn read_device_profile(&self, name: ProfileName) -> Result<Profile, DataError>;
    fn read_file_profile(&self, path: PathBuf) -> Result<Profile, DataError>;
    fn list_community(&self) -> Result<Vec<CommunityEntry>, DataError>;
    fn fetch_community(&self, url: String) -> Result<Profile, DataError>;
}
```

No serde, no `async-trait`, no DTOs — the trait passes `yoke-config` / `yoke-volume` domain types directly. `DataError` is a local `thiserror` enum (the UI's failure vocabulary), distinct from each library's own error type.

### Threading model

egui is immediate-mode: `App::update` runs every frame and must never block. I/O is therefore pushed off the UI thread.

- **Native:** a background **worker** owns the `DataSource` and a tokio runtime (multi-thread; `yoke-index` uses async `reqwest`). The UI sends `AppCommand`s over a channel; the worker performs the operation and returns a `DataEvent` (`ProfilesListed`, `ProfileOpened`, `CommunityListed`, `VolumeChanged`, `Failed { .. }`). The worker subscribes to `yoke-volume`'s `watch::Receiver<VolumePresence>` and emits `VolumeChanged` on every transition. On each delivered event the worker calls `egui::Context::request_repaint` so the UI wakes even when idle.
- **wasm:** there are no threads. The same `AppCommand` / `DataEvent` channel types are driven by an **inline synchronous pump** over `MockDataSource` (mock data is in-memory, so operations complete immediately). Volume presence emits one `Present` event at startup.

Because both targets speak the identical `AppCommand` / `DataEvent` protocol, all UI and view code is `cfg`-free. The native file dialog uses `rfd` (which also has a wasm path) invoked from the worker.

### Core / render separation (forward path to a shared `yoke-app` crate)

We chose the single-crate `DataSource`-trait architecture now, but build it so that a later extraction of a frontend-agnostic core crate (`yoke-app`) is a module move, not a rewrite. The discipline, enforced by review:

> The `data`, `state`, `stations`, and command/event modules import **zero** `egui`/`eframe`. Only `app.rs` and `views/` touch egui.

These egui-free modules hold the `DataSource` trait and impls, the open-profile state machine, the `AppCommand` / `DataEvent` protocol, and the station-layout/binding-filter logic. Lifting them into `yoke-app` later requires no change to their internals; `yoke-gui` would keep only the eframe entry points and the render layer.

### View shell and components

Single window, no router. A top-level enum drives the two views; egui panels lay out the regions.

```
YokeApp (impl eframe::App)
├── TopBottomPanel (top)      title text + DEVICE status pill (Connected / Disconnected / Backend error)
└── CentralPanel
    ├── SidePanel (left)      rail: "Profiles" entry + DEVICE section (live volume state)
    └── central
        ├── library view      (when open_profile is None)
        │   ├── device profile list   (list_device_profiles)
        │   ├── open-file button       (rfd -> read_file_profile)
        │   └── community list         (list_community; inline retry on failure)
        └── editor view       (when open_profile is Some)
            ├── header                 back action + breadcrumb + filename + metadata
            ├── sub-profile strip      selectable chips
            ├── device map             custom-painted egui sketch, Map variant only
            └── bindings panel         filtered by selected station; BindingRow list
```

The device map is drawn with egui's `Painter` from a static station table (see the device-map decision below). Clicking a station node sets `selected_station`; the bindings panel filters via an `input_belongs_to` mapping from `yoke-config`'s `Input` enum to the named station. Escape steps back one level: clear the station selection (revealing the ALL-bindings view), then close the profile and return to the library.

### State

Held in `YokeApp`, not in reactive signals (egui is immediate-mode — state is plain fields read each frame):

| Field | Type | Source |
|---|---|---|
| `volume` | `VolumePresence` | latest `VolumeChanged` event |
| `device_profiles` | `Vec<ProfileEntry>` | `ProfilesListed` event |
| `community` | `CommunityLoad` (`Loading` / `Loaded` / `Failed`) | `CommunityListed` / `Failed` events |
| `open_profile` | `Option<OpenProfile>` | set by open actions; cleared by back/Escape |
| `selected_station` | `Option<StationId>` | device-map clicks |
| `selected_subprofile` | `usize` | sub-profile strip |
| `toast` | `Option<(String, Instant)>` | errors; auto-dismiss ~5 s |

`OpenProfile { source: ProfileSource, profile: Profile }`, `ProfileSource = Device(ProfileName) | File(PathBuf) | Community { name, url }`. The editor consults `source` only for the breadcrumb; everything else is source-agnostic. The `CommunityLoad` three-state enum keeps "loading" / "loaded-empty" / "failed" distinct so the library never spins forever on a failed fetch.

### Chrome and theming

- **Chrome:** the real OS title bar (`eframe` native decorations on). No faux traffic lights. The window title is set; the DEVICE status pill lives in the in-app top panel.
- **Theme:** the design-handoff Console (dark) tokens (`--bg-0..4`, `--ink-1..3`, `--line`, `--accent`, `--accent-2`, category colors) translate to an `egui::Visuals` builder plus a small `Palette` struct for accent/category colors used by custom painting. Adding Studio/Contrast later is a second palette, not a component change.
- **Fonts:** Manrope (UI sans), JetBrains Mono (mono), Instrument Serif (display) embedded via `include_bytes!` + `FontDefinitions`, so the binary has no runtime font-fetch dependency.

### Reuse

The GUI builds on existing crates; it does not duplicate their logic:

- `yoke-config` — Profile model, `parse`, catalog (`Input`, `MpPosition`, ...). Consumed directly.
- `yoke-volume` + `yoke-volume-macos` — `VolumeProvider`, `FsBackend`, and the `watch::Receiver` presence channel. Called from `NativeDataSource`.
- `yoke-index` — community index fetch + TTL cache. Called from `NativeDataSource`.

Two small pieces are authored in `yoke-gui` from the design reference rather than reused from a library:

- **Station layout + `input_belongs_to`** — a static table of the eight stations (coordinates, labels, kind) plus the mapping from `yoke-config`'s `Input` variants to a station. Coordinates and labels come from the design handoff (`design_handoff_quadstick_config/src/data.js` and `device.jsx`). This is UI-only layout data and stays decoupled from any device-side station model.
- **Mock fixture** — one real CSV from the QuadStick corpus, committed at `crates/yoke-gui/fixtures/default.csv` and `include_str!`'d. One profile exercises the entire UI.

### Device map: native painting, not SVG (decided)

The map is drawn with egui's `Painter` rather than rendered from an SVG asset. egui has no native SVG; SVG support means pulling `egui_extras`'s `svg` feature, which adds the `resvg`/`usvg`/`tiny-skia` tree (and `fontdb` + system fonts for `svg_text`) — heavy for a native, self-contained stack, and it inflates the wasm dev bundle. The map is also data-driven (per-station binding-count badges, selected ring) and interactive (click to filter), neither of which a static rasterized SVG expresses without an egui overlay that re-derives the same geometry. `Painter` output is tessellated vector geometry — crisp at any DPI with no re-rasterization. The static station table keeps the painting bounded. Reconsider an SVG asset only for the deferred, illustrative "Diagram" sketch variant, where the content is static rather than a data visualization.

### Error handling

| Failure | UI surface |
|---|---|
| Parse error (any source) | Toast; editor does not open; library stays. |
| Read I/O error from device | Toast. |
| Community fetch network error | `CommunityLoad::Failed`; inline error row with a retry action. |
| File-dialog cancelled | No-op. |
| Volume backend init failed | Status pill shows "backend error" + tooltip; file-open and community still work. |

Toasts are a small egui overlay (bottom-right, ~5 s auto-dismiss). No third-party toast crate.

**Disconnect-mid-session (provisional):** if the volume unmounts while the editor is open, the editor stays open on the in-memory profile and the status pill flips to disconnected. Re-evaluate against the running UI before merging.

## On-disk layout

```
yoke/crates/yoke-gui/
├── Cargo.toml          # native deps target-gated off wasm32
├── index.html          # trunk entry (wasm dev)
├── Trunk.toml
├── assets/fonts/       # Manrope, JetBrains Mono, Instrument Serif (embedded)
├── fixtures/
│   └── default.csv     # committed mock fixture (one real corpus profile)
└── src/
    ├── main.rs         # native entry: eframe::run_native
    ├── lib.rs          # YokeApp + wasm entry (eframe::WebRunner, cfg wasm32)
    ├── app.rs          # YokeApp: state fields + per-frame dispatch (touches egui)
    ├── theme.rs        # tokens -> Visuals + Palette (touches egui)
    ├── views/          # library.rs, editor.rs, map.rs, bindings.rs (touch egui)
    ├── data/
    │   ├── mod.rs      # DataSource trait, DataError, AppCommand, DataEvent (egui-free)
    │   ├── native.rs   # NativeDataSource (cfg not wasm32, egui-free)
    │   └── mock.rs     # MockDataSource (egui-free)
    ├── worker.rs       # native: bg thread + tokio; wasm: inline pump (only cfg site)
    ├── stations.rs     # static station table + input_belongs_to (egui-free)
    └── state.rs        # OpenProfile, ProfileSource, CommunityLoad (egui-free)
```

## Dependencies

All adds via `cargo add` per [`AGENTS.md`](../../../AGENTS.md). Exact versions pinned at implementation; verify latest at that point.

- **Always:** `egui`, `eframe`, `tracing`, workspace dep on `yoke-config`. `serde` is **not** needed (no wire format).
- **Native only** (`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`): `tokio` (rt-multi-thread), `rfd`, `anyhow`, `tracing-subscriber`, workspace deps on `yoke-volume`, `yoke-index`; `yoke-volume-macos` under a further `cfg(target_os = "macos")` section.
- **wasm only:** `wasm-bindgen`, `wasm-bindgen-futures`, `console_error_panic_hook`, `tracing-wasm`.
- Candidate helpers (decide at implementation): `poll-promise` for one-shot async results, if the bare channel pattern proves noisy.

## Tooling and CI

- **devShell additions** (`flake.nix`): `trunk` and the matching `wasm-bindgen-cli`. egui native uses system frameworks already present on macOS; no extra GUI toolkit is required.
- **CI gates** (extend `.github/workflows/ci.yml`): `cargo build -p yoke-gui` (native), `cargo test -p yoke-gui` (host logic + mock), and `trunk build` inside `crates/yoke-gui` (wasm). No bundling/signing in this stage — packaging is a later concern.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `stations.rs` | station table + `input_belongs_to` | host `cargo test` |
| `DataSource` logic | source→breadcrumb, `CommunityLoad` transitions, mock behavior | host `cargo test` against `MockDataSource` |
| Build gates | native build, wasm `trunk build` | CI |

`egui_kittest` snapshot/visual-regression tests are **deliberately deferred** — not in this stage. Manual acceptance:

- native run with a fake/real volume populates the device list; clicking opens the editor.
- `trunk serve` reproduces the library and editor flows via `MockDataSource`.
- community list shows at least one entry in mock mode; click opens the editor.
- disconnect the volume mid-session: editor stays open, status pill updates (validate it feels right).

## Risks and open questions

- **egui ecosystem versions.** `egui`/`eframe`/`rfd` move fast; pin exact versions and the matching `wasm-bindgen-cli`. Mismatches surface as build errors, not silent breakage.
- **Async on the wasm pump.** Native uses tokio; the wasm build has no runtime. Mock data is synchronous so this is a non-issue for v1, but a future wasm build that does real fetches would need `wasm-bindgen-futures`. Out of scope now.
- **Accessibility.** egui exposes accessibility through AccessKit rather than the DOM. For a tool serving users with disabilities this matters and will be addressed; it is not scored in the read-only v1 but is on the radar before shipping for real.
- **Disconnect-keeps-editor behavior.** Provisional; validate in the running UI.

## References

- Design handoff: `../../../../design_handoff_quadstick_config/README.md` and the JSX prototypes in `src/` (visual reference; not a port target). Station coordinates: `design_handoff_quadstick_config/src/data.js`, `device.jsx`.
- Existing crates reused by this stage:
  - [`yoke-config`](../../../crates/yoke-config) — Profile model, dual-target.
  - [`yoke-volume`](../../../crates/yoke-volume) — `VolumeProvider` trait + `FsBackend` + `watch::Receiver` presence.
  - [`yoke-volume-macos`](../../../crates/yoke-volume-macos) — DiskArbitration/IOKit impl.
  - [`yoke-index`](../../../crates/yoke-index) — community index fetch + TTL cache.
- egui / eframe: <https://github.com/emilk/egui>. Web deployment via trunk: <https://github.com/emilk/eframe_template>.
