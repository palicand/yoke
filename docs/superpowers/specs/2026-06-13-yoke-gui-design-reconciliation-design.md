# Yoke GUI — design reconciliation + quirk fixes

**Date:** 2026-06-13
**Stage:** F (follow-up)
**Status:** proposed
**Predecessors:** [`2026-05-27-yoke-gui-egui-design.md`](2026-05-27-yoke-gui-egui-design.md), [`2026-05-30-yoke-gui-editor-design.md`](2026-05-30-yoke-gui-editor-design.md)

## Goal

Close the gap between the shipped egui GUI and the visual design handoff, and
fix two concrete UX quirks. The shipped app is functionally complete (library +
editor + picker) but visually bare next to the handoff. This pass restyles every
screen toward the handoff's Console (dark) theme and fixes a resize stall and a
rail label that wraps.

The work ships as a stack of small, independently reviewable PRs, each of which
keeps `clippy -D warnings`, `fmt`, `cargo test`, and the wasm `trunk build`
green on its own.

## Non-goals / constraints

These bound "reconcile as much as possible":

- **Console theme only.** The handoff defines Studio (light), Console (dark),
  and Contrast palettes. Only Console is implemented today and every reference
  screenshot uses it; Studio/Contrast stay deferred (consistent with the egui
  spec).
- **No faux window chrome.** The handoff mock draws fake macOS traffic lights
  and a centered "QuadStick · Configuration" title bar *inside the page*. The
  egui spec deliberately uses the real OS title bar; that decision stands. We
  keep Yoke's real title bar and in-app top strip. We **do** add the handoff's
  bottom status bar (it does not conflict).
- **No fabricated data.** Device profile entries carry only
  `{ name, label }` ([`ProfileEntryView`](../../../crates/yoke-gui/src/data/mod.rs)).
  The handoff cards show free-text descriptions, "last used N days ago", and
  play counts — none of which exist in a QuadStick CSV. We do **not** invent
  them. We derive what is real (see "Profile metadata") and leave the rest off.
- **Each PR is green standalone.** Shared theme primitives land in the same PR
  as their first consumer, because the workspace lints at `-D warnings` and an
  unused helper would fail the build.

## The two quirks

### Resize hang

The root cause is the wgpu present mode, not egui layout — a native `sample`
during a resize hang put 56% of main-thread time blocked in
`[CAMetalLayer nextDrawable]` (`_dispatch_semaphore_wait`) inside AppKit's
synchronous live-resize modal loop, with egui layout a rounding error. eframe
defaults the surface to [`present_mode: AutoVsync`][present-mode], which on
Metal is `Fifo`; under `Fifo`, `get_current_texture` blocks on the vsync-paced
drawable queue, and during the resize loop the OS drives redraws faster than
vsync releases drawables, so `nextDrawable` stalls (the same class of stall wgpu
documents for the [occluded-window case][wgpu-4779]). Fix: set
`wgpu_options.present_mode = AutoNoVsync` in [`main.rs`](../../../crates/yoke-gui/src/main.rs),
which wgpu-hal maps to Metal `Immediate` (`displaySyncEnabled = false`) on
displays that support it, so acquiring a drawable never waits for vsync. The app
is reactive, so there is no idle GPU cost. (The `Arc<Vec<IndexEntry>>` community
snapshot and the 48-card display cap remain as per-frame-cost hygiene, but they
are not the resize fix.)

### Rail status label wraps

`rail_device_status` ([`app.rs`](../../../crates/yoke-gui/src/app.rs)) renders
the device-status text (e.g. `"Connected - emulation mode"`) in a fixed
non-resizable rail. The rail was also collapsing to its content width: a
non-resizable `egui::Panel` stores its rendered rect by id every frame and reads
it back, so `default_size` only seeds the first frame and the rail then shrinks
to fit `"Connected"`, truncating the longer `DeviceVisibleNoVolume` labels. Fix:
pin the rail with `exact_size(260.0)` (which sets the panel's size range to a
point, forcing the width every frame) — wide enough that the longest status
label never wraps or truncates.

## Profile metadata (data layer)

To make the library cards honest and the kind filter real, the worker enriches
device profile entries on listing. `ProfileEntryView` gains optional derived
fields:

- `kind` — derived from the profile's sub-profile modes (Mouse/Keyboard-leaning
  vs Gamepad vs Mixed), mapped to the handoff's `Mouse + Keys` / `Gamepad` /
  `Mixed` tags.
- `bindings` — total binding count.
- `sub_profiles` — sub-profile count.
- `modes` — sub-profile mode names, used as the card's footer chips.

These are populated by parsing each device profile when
`list_device_profiles` runs on the worker thread (the device holds only a
handful of small CSVs, so the parse cost is negligible and off the UI thread).
Community entries (`yoke_index::IndexEntry`) keep their existing shape; a `kind`
is shown only when the index `fields` map carries one, otherwise the community
card shows name only. The authoritative shape lives in
[`data/mod.rs`](../../../crates/yoke-gui/src/data/mod.rs); the derivation lives
next to `NativeDataSource`.

The kind derivation is also exercised on wasm via `MockDataSource` so the
browser dev build renders the same cards.

## Per-screen design

Token names below are the handoff `[data-theme="console"]` CSS custom
properties; their egui equivalents already live in
[`theme.rs`](../../../crates/yoke-gui/src/theme.rs) (`Palette` + the `BG_*` /
`LINE*` consts). New reusable primitives (eyebrow text, kind badge, primary
button, segmented control, search field, status bar) are added to `theme.rs`
alongside the screen that first uses them.

### Library (`.lib-*`)

- **Header (`.lib-hd`):** mono uppercase eyebrow "PROFILE LIBRARY"; serif italic
  display title "Your profiles" (Instrument Serif, ~40px); subtitle
  "N on QuadStick · <mount state>". Right-aligned actions: "Import .csv" (ghost
  button → existing file-open dialog). "New profile" is deferred (see below).
- **Toolbar (`.lib-toolbar`):** search pill (live, client-side name filter);
  segmented kind filter All / Mouse + Keys / Gamepad / Mixed; right-aligned
  "Sorted by name" hint; bottom hairline.
- **Device grid (`.lib-grid`):** three-column grid of fixed-height
  `.prof-card`s. Each card: kind tag (colored per `.kind-tag.{gamepad,mk}`),
  filename (mono, muted, top-right), name, footer with mode chips
  (`.sub-chip`) + counts. Clicking opens the profile.
- **Community panel:** the handoff mock has no community list, but the real
  index is hundreds of entries. Community renders below the device grid inside a
  single `ScrollArea` that covers the whole library body (device grid + community
  grid). The community list is filtered by search and kind first, then display is
  capped at `LIB_COMMUNITY_DISPLAY_CAP` (48) cards per frame; if the filtered
  count exceeds the cap a muted "Showing N of M — refine search to narrow" note
  is shown. Search reaches every entry; the cap limits only the rendered slice
  (filtered or not), never the search itself.
  This keeps per-frame cost bounded and the resize smooth while keeping the
  community section reachable below the device grid.
- **Empty/loading/failed/disabled** community states keep their current
  semantics ([`CommunityLoad`](../../../crates/yoke-gui/src/state.rs)); only the
  styling changes.

### Editor header + sub-profile strip (`.ed-top`, `.sub-tabs`)

- **Header:** back button (`.back-btn`); mono breadcrumb + "EDITING PROFILE"
  eyebrow; serif italic title (the profile title); stat row (`.ed-stats`):
  "N bindings", "M sub-profiles", and an amber "unsaved" stat when dirty.
  Right-aligned toolbar (`.ed-actions`): Undo/Redo, Preview CSV, Save / Save
  As / Save to QuadStick — same actions as today, restyled (ghost buttons +
  one primary "Save to QuadStick").
- **Sub-profile strip (`.sub-tabs`):** mono "SUB-PROFILE" label; selectable
  chips each showing an index badge (`.sub-tab-i`, e.g. `L1`), the mode name,
  a mode sublabel, and the binding count (`.sub-tab-count`); a dashed
  "Add layer" chip (`.sub-tab.add`). The existing rename / clone / delete
  management affordances and inline forms are **preserved** (the app is an
  editor, beyond the read-only mock) — they are restyled, not removed.

### Editor panes — bindings + map (`.bind-pane`, `.dev-pane`)

- **Bindings rows:** restyle to the handoff `.evt-row` / `.brow` layout — a
  short-code glyph box, a WHEN block (eyebrow + input), a modifier pill, an
  arrow, an output button carrying a category-colored pill, and a clear "x".
  Both the station-filtered roster and the unfiltered "All bindings" list adopt
  this row shape. Output category color comes from
  [`output_color`](../../../crates/yoke-gui/src/theme.rs).
- **Device map (`.dev-pane`/`.dev-svg-wrap`):** the painted map
  ([`views/map.rs`](../../../crates/yoke-gui/src/views/map.rs)) keeps its
  geometry; the pane gains the handoff's dotted-grid framing, the `dev-meta`
  header row, and a station legend row. Painting stays native (per the egui
  spec's "native painting, not SVG" decision).

### Action picker (`.picker-*`)

Restyle the existing `egui::Modal`
([`views/picker.rs`](../../../crates/yoke-gui/src/views/picker.rs)) to the
handoff modal: title + close affordance, a framed search field, category chips
as a chip row, and dense list rows (glyph + label + category tag). The
key-capture banner adopts `.kc-banner` styling (armed = accent-tinted). All
existing commit/guard logic (duplicate pre-check, unrecognized-modifier refusal)
is untouched — visual only.

### Status bar (`.status`)

A bottom strip (mono, ~11px, `--bg-3` fill, top hairline): a sync dot + state,
the mount path when present, and the device/community profile counts. Pure
display of state the app already holds; no new data.

## The stack

Bottom → top. Branch names are `gui/NN-slug`; each row is one PR.

| # | Branch | Summary |
|---|--------|---------|
| 0 | `gui/00-spec` | This spec. |
| 1 | `gui/01-rail-status-wrap` | The two app-shell quirks: rail width pinned (`exact_size`) so labels never wrap/truncate, and `AutoNoVsync` present mode to stop the macOS resize hang. |
| 2 | `gui/02-profile-metadata` | Enrich `ProfileEntryView` (kind/counts/modes) by parsing device profiles on list; mock parity; filter `prefs.csv` (device settings, not a profile) out of the device list. |
| 3 | `gui/03-library` | Library redesign; single scrollable body + 48-card community display cap; functional search / kind filter / Import. |
| 4 | `gui/04-editor-header-strip` | Editor header + sub-profile strip styling. |
| 5 | `gui/05-editor-panes` | Binding rows + device-map pane framing. |
| 6 | `gui/06-picker` | Action picker modal styling. |
| 7 | `gui/07-status-bar` | Bottom status bar + top-strip alignment. |

## Deferred

- **New profile / Blank profile** — creating a profile from scratch is a real
  feature (empty `EditSession` + first save), not styling. Out of this stack;
  trivial to add as a follow-up PR.
- **Studio / Contrast themes**, **Diagram / Grid sketch variants**, a
  **tweaks panel** — all already deferred by the egui spec; unchanged here.

## Testing

- `profile-metadata`: unit tests for the kind derivation (mode set → kind) and
  count fields, against fixture CSVs.
- View code stays largely untested (egui rendering), as in the egui spec;
  `egui_kittest` snapshot testing remains deferred. Pure helpers (kind
  derivation, label/format functions) are unit-tested.
- Every PR runs the full gate set in CI: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo build`/`test -p yoke-gui`, and the wasm `trunk build`.

## References

- Design handoff: `design_handoff_quadstick_config/` in the parent workspace
  (`QuadStick App.html`, `src/app.jsx`, `src/device.jsx`, `src/picker.jsx`) —
  visual reference, not a port target.
- egui / eframe 0.34 non-deprecated forms: see the egui spec's implementation
  notes and [`app.rs`](../../../crates/yoke-gui/src/app.rs) /
  [`theme.rs`](../../../crates/yoke-gui/src/theme.rs).

[present-mode]: https://wgpu.rs/doc/wgpu_types/enum.PresentMode.html
[wgpu-4779]: https://github.com/gfx-rs/wgpu/issues/4779
