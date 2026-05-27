# Yoke GUI (egui) — read-only viewer, competing frontend

**Date:** 2026-05-27
**Stage:** E (alternative implementation)
**Status:** proposed
**Predecessors:** [`2026-05-16-yoke-scaffold-design.md`](2026-05-16-yoke-scaffold-design.md), [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-17-yoke-volume-design.md`](2026-05-17-yoke-volume-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)
**Counterpart:** `2026-05-25-yoke-ui-v1-design.md` (the Tauri 2 + Leptos viewer, on branch `stage-e-ui-v1`). This document is its egui sibling and is meant to be compared against it head-to-head.

## Goal

Build an egui frontend that reaches **exact functional parity** with the Tauri v1 read-only profile viewer, so the two approaches can be evaluated side by side and one chosen as the path forward.

The motivation is architectural, not feature-driven: the Tauri shell ships a full WebView as its production runtime. For an all-Rust desktop app that reads local CSV files and talks to a USB device, embedding a browser engine to render the UI is a heavy, non-native dependency. egui renders natively (via `eframe`/`wgpu` or glow) with no WebView in the shipped binary. This branch tests whether egui delivers the same UX with a smaller, more native, more self-contained stack.

This is a bake-off branch. It is based on the same commit as `stage-e-ui-v1` (`origin/main`) so the comparison is fair, and it lands its own evaluation record (see "Comparison artifact").

## Scope: parity with the Tauri viewer

Same in-scope and out-of-scope set as the v1 viewer. The user can open a profile from the mounted QuadStick volume, from a local CSV file, or from the community index, and see the three-region layout (rail + sub-profile strip + device map + bindings panel). Clicking a station filters the bindings panel. Live volume presence drives a status indicator.

### In v1

- Volume-first launch: poll `yoke-volume` for a mounted QuadStick; device profiles populate the library when present.
- File-open fallback: a native file picker opens any local CSV.
- Community profiles: list entries via `yoke-index`.
- Three-region editor layout, dark theme, Map sketch view.
- Single rail entry "Profiles" plus a live DEVICE status section.

### Deliberately not in v1

Identical to the Tauri spec's cut list: no action picker, no key-capture, no save / edit, no tweaks panel, no rail items beyond Profiles + DEVICE, no live content refresh, no Diagram/Grid sketch variants, no Studio/Contrast themes, no Windows/Linux, no recent-files. `yoke-edit` is **not** a dependency — this stage reads, it does not write.

## Architecture

### The collapse vs Tauri

egui runs natively in the host process, so it calls the library crates directly. The entire IPC layer the Tauri design needed to bridge a WebView to Rust **disappears**:

| Present in Tauri v1 | Status in egui |
|---|---|
| `yoke-ipc` crate (serde wire DTOs) | **Dropped.** The UI consumes `yoke-config` / `yoke-volume` types directly. No serialization boundary. |
| `yoke-tauri` host process (commands, capabilities, `tauri.conf.json`, signing) | **Dropped.** There is one process. |
| `Backend` trait + `TauriBackend` + `tauri-sys` `invoke()` round-trip | **Replaced** by an in-process `DataSource` trait (no `invoke`, no JSON). |
| `WindowChrome` faux macOS traffic lights | **Dropped.** Real OS title bar (see "Chrome"). |
| Google-Fonts-at-runtime | **Dropped.** Fonts embedded in the binary. |

### New crate

One new crate, `yoke-gui` — a single binary that *is* the app. No host/frontend split exists because there is no IPC seam to split across.

`eframe` compiles the same source to two targets:
- **native** (`cargo run -p yoke-gui`): the shipped app. Real window, real device, fs, network.
- **wasm** (`trunk serve` inside `crates/yoke-gui`): a browser dev build serving the `MockDataSource`. This preserves the AGENTS.md substrate rule — an agent that cannot see a native window iterates in a browser exactly as it does today with `trunk serve`. The shipped native binary contains **no WebView**; the browser build is a development convenience only, not the production runtime.

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

- **Native:** a background **worker** owns the `DataSource` and a tokio runtime (multi-thread; `yoke-index` uses async `reqwest`). The UI sends `AppCommand`s over a channel; the worker performs the operation and returns a `DataEvent` (`ProfilesListed`, `ProfileOpened`, `CommunityListed`, `VolumeChanged`, `Failed { .. }`). The worker subscribes to `yoke-volume`'s `watch::Receiver<VolumePresence>` and emits `VolumeChanged` on every transition — no event-bridging shim is needed (the Tauri design had to re-emit volume state as Tauri events; here the watch channel is consumed directly). On each delivered event the worker calls `egui::Context::request_repaint` so the UI wakes even when idle.
- **wasm:** there are no threads. The same `AppCommand` / `DataEvent` channel types are driven by an **inline synchronous pump** over `MockDataSource` (mock data is in-memory, so operations complete immediately). Volume presence emits one `Present` event at startup.

Because both targets speak the identical `AppCommand` / `DataEvent` protocol, all UI and view code is `cfg`-free. The native file dialog uses `rfd` (which also has a wasm path) invoked from the worker.

### Core / render separation (forward path to a shared `yoke-app` crate)

We chose the single-crate `DataSource`-trait architecture now, but build it so that a later extraction of a frontend-agnostic core crate (`yoke-app`) is a module move, not a rewrite. The discipline, enforced by review:

> The `data`, `state`, `stations`, and command/event modules import **zero** `egui`/`eframe`. Only `app.rs` and `views/` touch egui.

These egui-free modules hold the `DataSource` trait and impls, the open-profile state machine, the `AppCommand` / `DataEvent` protocol, and the station-layout/binding-filter logic. Lifting them into `yoke-app` later requires no change to their internals; `yoke-gui` would keep only the eframe entry points and the render layer. This mirrors the platform-isolation discipline the Tauri spec applied to `cfg(target_os)`.

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

The device map is drawn with egui's `Painter` from the static station table (ported from the Tauri build's `stations.rs`) rather than an SVG asset (see the device-map decision below). Clicking a station node sets `selected_station`; the bindings panel filters via the same `input_belongs_to` logic. Escape steps back one level (clear station selection, then close profile) — matching the v1 interaction addendum.

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

`OpenProfile { source: ProfileSource, profile: Profile }`, `ProfileSource = Device(ProfileName) | File(PathBuf) | Community { name, url }`. The editor consults `source` only for the breadcrumb; everything else is source-agnostic. The `CommunityLoad` three-state enum is carried over from v1 — it keeps "loading" / "loaded-empty" / "failed" distinct so the library never spins forever on a failed fetch.

### Chrome and theming

- **Chrome:** the real OS title bar (`eframe` native decorations on). No faux traffic lights. The window title is set; the DEVICE status pill lives in the in-app top panel. This is more native and is the whole point of the branch.
- **Theme:** the design-handoff Console (dark) tokens (`--bg-0..4`, `--ink-1..3`, `--line`, `--accent`, `--accent-2`, category colors) translate to an `egui::Visuals` builder plus a small `Palette` struct for accent/category colors used by custom painting. Adding Studio/Contrast later is a second palette, not a component change.
- **Fonts:** Manrope (UI sans), JetBrains Mono (mono), Instrument Serif (display) embedded via `include_bytes!` + `FontDefinitions`. This removes v1's first-run network dependency on Google Fonts.

### Reuse vs rewrite

| Reused directly | Rewritten for egui |
|---|---|
| `yoke-config` (Profile, `parse`, catalog: `Input`, `MpPosition`, ...) | Every view/widget — immediate-mode painting, not Leptos components |
| `yoke-volume` + `yoke-volume-macos`, `yoke-index` (called directly) | The data seam — `DataSource` trait, no IPC/serde |
| `stations.rs` — `STATIONS` table + `input_belongs_to` + tests (ported verbatim from `stage-e-ui-v1:crates/yoke-ui/src/components/stations.rs`) | Theme — CSS variables to `egui::Visuals` + `Palette` |
| `default.csv` fixture (from `stage-e-ui-v1:crates/yoke-ui/fixtures/default.csv`) | Window chrome — native title bar |
| `CommunityLoad` state shape, source→breadcrumb derivation (pure logic, ported) | Device map — SVG to egui `Painter` |

### Error handling

| Failure | UI surface |
|---|---|
| Parse error (any source) | Toast; editor does not open; library stays. |
| Read I/O error from device | Toast. |
| Community fetch network error | `CommunityLoad::Failed`; inline error row with a retry action. |
| File-dialog cancelled | No-op. |
| Volume backend init failed | Status pill shows "backend error" + tooltip; file-open and community still work. |

Toasts are a small egui overlay (bottom-right, ~5 s auto-dismiss). No third-party toast crate.

**Disconnect-mid-session:** as in v1 (provisional) — if the volume unmounts while the editor is open, the editor stays open on the in-memory profile and the status pill flips to disconnected. Re-evaluate against the running UI before merging.

## On-disk layout

```
yoke/crates/yoke-gui/
├── Cargo.toml          # native deps target-gated off wasm32
├── index.html          # trunk entry (wasm dev)
├── Trunk.toml
├── assets/fonts/       # Manrope, JetBrains Mono, Instrument Serif (embedded)
├── fixtures/
│   └── default.csv     # committed mock fixture (ported)
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
    ├── stations.rs     # ported verbatim (egui-free)
    └── state.rs        # OpenProfile, ProfileSource, CommunityLoad (egui-free)
```

## Dependencies

All adds via `cargo add` per [`AGENTS.md`](../../../AGENTS.md). Exact versions pinned at implementation; verify latest at that point.

- **Always:** `egui`, `eframe`, `tracing`, workspace dep on `yoke-config`. `serde` is **not** needed (no wire format).
- **Native only** (`[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`): `tokio` (rt-multi-thread), `rfd`, `anyhow`, `tracing-subscriber`, workspace deps on `yoke-volume`, `yoke-index`; `yoke-volume-macos` under a further `cfg(target_os = "macos")` section (mirrors the Tauri spec's platform-isolation rule).
- **wasm only:** `wasm-bindgen`, `wasm-bindgen-futures`, `console_error_panic_hook`, `tracing-wasm`.
- Candidate helpers (decide at implementation): `poll-promise` for one-shot async results, if the bare channel pattern proves noisy.

## Tooling and CI

- **devShell additions** (`flake.nix`): `trunk` and the matching `wasm-bindgen-cli`. `eframe` native needs the platform's GL/WebKit-free windowing stack — on macOS that is system frameworks already present; no `cargo-tauri` required.
- **CI gates** (extend `.github/workflows/ci.yml`): `cargo build -p yoke-gui` (native), `cargo test -p yoke-gui` (host logic + mock), and `trunk build` inside `crates/yoke-gui` (wasm). No bundling/signing step in this branch — packaging parity is a later concern and is not part of the viewer comparison.

## Comparison artifact

Because the point is a decision, the branch records a head-to-head against `stage-e-ui-v1`. Captured in this spec's eventual addendum (or a short `docs/` note), not as committed code:

- shipped binary size (no WebView vs Tauri bundle)
- direct + transitive dependency count and build time (cold/warm)
- lines of code for equivalent functionality
- native-feel and input latency (subjective, noted)
- agent-iteration parity (does `trunk serve` give the same browser loop?)
- accessibility posture (egui AccessKit vs DOM/WebView a11y) — flagged, not scored, in v1

The winner's crate may be renamed to `yoke-ui` and the loser's branch dropped; that decision is out of scope for this spec.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `stations.rs` | station table + `input_belongs_to` | ported tests, host `cargo test` |
| `DataSource` logic | source→breadcrumb, `CommunityLoad` transitions, mock behavior | host `cargo test` against `MockDataSource` |
| Build gates | native build, wasm `trunk build` | CI |

`egui_kittest` snapshot/visual-regression tests are **deliberately deferred** — not in this branch. Manual acceptance mirrors v1:

- native run with a fake/real volume populates the device list; clicking opens the editor.
- `trunk serve` reproduces the library and editor flows via `MockDataSource`.
- community list shows at least one entry in mock mode; click opens the editor.
- disconnect the volume mid-session: editor stays open, status pill updates (validate it feels right).

**Mock fixture:** one real corpus CSV at `crates/yoke-gui/fixtures/default.csv`, `include_str!`'d. One profile exercises the whole UI.

## Risks and open questions

- **egui ecosystem versions.** `egui`/`eframe`/`rfd` move fast; pin exact versions and the matching `wasm-bindgen-cli`. Mismatches surface as build errors, not silent breakage.
- **Async on the wasm pump.** Native uses tokio; the wasm build has no runtime. Mock data is synchronous so this is a non-issue for v1, but a future wasm build that does real fetches would need `wasm-bindgen-futures`. Out of scope now.
- **Device map: native painting, not SVG (decided).** The map is drawn with egui's `Painter` rather than rendered from an SVG asset. egui has no native SVG; SVG support means pulling `egui_extras`'s `svg` feature, which adds the `resvg`/`usvg`/`tiny-skia` tree (and `fontdb` + system fonts for `svg_text`) — heavy for a branch whose premise is a lean, WebView-free stack, and it inflates the wasm dev bundle. The map is also data-driven (per-station binding-count badges, selected ring) and interactive (click to filter), neither of which a static rasterized SVG expresses without an egui overlay that re-derives the same geometry. `Painter` output is tessellated vector geometry — crisp at any DPI with no re-rasterization. The static `stations.rs` table keeps the painting bounded. Reconsider an SVG asset only for the deferred, illustrative "Diagram" sketch variant, where the content is static rather than a data visualization.
- **Accessibility.** egui exposes accessibility through AccessKit rather than the DOM. For a tool serving users with disabilities this matters; it is not scored in the read-only v1 but must be on the radar before either approach ships for real.
- **Disconnect-keeps-editor behavior.** Provisional, same as v1.

## References

- Counterpart spec: `2026-05-25-yoke-ui-v1-design.md` (Tauri 2 + Leptos viewer), on branch `stage-e-ui-v1`.
- Design handoff: `../../../../design_handoff_quadstick_config/README.md` and the JSX prototypes in `src/` (visual reference; not a port target).
- Existing crates reused by this stage:
  - [`yoke-config`](../../../crates/yoke-config) — Profile model, dual-target.
  - [`yoke-volume`](../../../crates/yoke-volume) — `VolumeProvider` trait + `FsBackend` + `watch::Receiver` presence.
  - [`yoke-volume-macos`](../../../crates/yoke-volume-macos) — DiskArbitration/IOKit impl.
  - [`yoke-index`](../../../crates/yoke-index) — community index fetch + TTL cache.
- Ported source (on branch `stage-e-ui-v1`): `crates/yoke-ui/src/components/stations.rs`, `crates/yoke-ui/fixtures/default.csv`.
- egui / eframe: <https://github.com/emilk/egui>. Web deployment via trunk: <https://github.com/emilk/eframe_template>.
