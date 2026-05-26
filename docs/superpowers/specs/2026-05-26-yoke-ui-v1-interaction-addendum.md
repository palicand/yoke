# Yoke UI v1 — interaction & native-feel addendum

**Date:** 2026-05-26
**Stage:** E
**Status:** approved
**Amends:** [`2026-05-25-yoke-ui-v1-design.md`](2026-05-25-yoke-ui-v1-design.md)
**Related:** [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md) § 5.3 (community URL transform)

Refinements found while exercising the v1 viewer in the live Tauri shell. Each
item amends a specific section of the base spec; everything else there stands.

## 1. Native-feel chrome

The base spec describes the visual theme but not the desktop-app feel. Two
additions, both so the window behaves like a native app rather than a web page:

- **No text selection.** `body` carries `user-select: none` (and the
  `-webkit-` prefix WKWebView needs); `input`, `textarea`, and
  `[contenteditable="true"]` re-enable it for genuine text entry. Chrome,
  labels, list items, and the device sketch are not selectable.
- **No right-click menu in release builds.** A `contextmenu` window listener
  calls `preventDefault()` so WKWebView's native "Reload" / "Back" menu never
  appears. Gated on `cfg!(debug_assertions)`: **debug builds keep the menu**
  (`trunk serve`, `tauri dev`) so dev tooling stays reachable; release builds
  suppress it. The listener lives for the whole session.

Both live in `yoke-ui` (CSS token sheet + a boot-time effect). No new
dependency, no `cfg(target_os)` — consistent with the platform-isolation rules.

- **Themed scrollbars.** Scrollable surfaces (bindings panel, library/main,
  rail, sub-profile strip) use a thin dark-themed scrollbar styled **only** via
  the `::-webkit-scrollbar*` pseudo-elements. The standard `scrollbar-width` /
  `scrollbar-color` properties are deliberately **not** set: per MDN, a non-auto
  value of either suppresses the `::-webkit-scrollbar` pseudo-elements in
  WebKit/Blink, and WKWebView's standard-property rendering falls back to a
  near-native bar that clashes with the dark UI. Do not reintroduce the standard
  properties — they re-break the WKWebView scrollbar.

## 2. Editor interaction model

The base spec's State table (`selected_input`, "mutated by device-sketch clicks
and event-chip clicks") under-specified how a selection is *cleared* and how the
keyboard navigates. Pinned behavior:

- **Station toggle.** Clicking the already-selected station clears
  `selected_input` (panel returns to the `ALL` view listing every binding).
  Previously a second click re-selected the same station with no way back to
  `ALL` short of opening a different profile.
- **Escape steps back one level.** A window `keydown` listener owned by
  `EditorView`:
  - if a station is selected → clear it (reveal `ALL`);
  - else → set `open_profile = None` (return to the library).

  This makes Escape the keyboard complement to the station toggle and the
  header back-button (base spec "Back to library", which still works). The
  listener is registered with `window_event_listener` and removed in
  `on_cleanup` when the editor unmounts, so it never double-fires across
  open/close cycles. The library is the root view; Escape there is a no-op.

The general principle for later views: **Escape always goes to the previous
state/view.** New nested selection state added in Stage F should clear on
Escape before the view itself closes.

## 3. Device-profile refresh on mass-storage enable

Amends the base spec's "Mounted while in library → `device_profiles` refetches."
That single refetch is fired by the `Present` volume event, and on real macOS
hardware it could land before the freshly-mounted FAT directory was enumerable —
leaving the list empty until the user reloaded the app.

Root cause: macOS publishes `Present` the instant the volume mounts, but the
directory takes a moment to become listable. The mount state then does **not**
change again, so no second volume event arrives — the one refetch is the only
chance. The provider's 1 s USB poll re-publishes the *same* `Present` state,
which `watch::send_if_modified` suppresses (no transition → no event).

Fix (frontend, `effects.rs`): on the `Present` transition, re-list device
profiles with a bounded settle-retry — up to 5 attempts, 200 ms apart, stopping
early on a non-empty result, and bailing if the volume is no longer present.
A genuinely empty volume settles to an empty list after the window. A listing
**error** is now surfaced via the toast (and `tracing::error!`) instead of being
silently swallowed.

This is a frontend-only mitigation for the mount race; it does not change the
`VolumeProvider` contract. Hardware-side verification still belongs to a manual
acceptance pass with a real QuadStick.

**Scope boundary (E vs F).** The retry settles the *first* read after the
volume appears — it is not live-refresh. External drift while the volume stays
mounted (a profile added via Finder / `cp` / `yokectl install`, or the open
profile's bytes changing on disk) is **not** detected; the user must re-open to
reload. This is the base spec's existing deferral ("Watching the volume
*contents* vs presence is Stage F territory once edit/save is in"). Polling the
listing on an interval was considered and rejected: it would refresh only the
library list (not the editor's loaded profile), spin the FAT volume / wake the
device periodically, and front-run that deferral. Stage F should instead watch
mount-point contents via FSEvents and emit a "contents changed" signal that
refreshes the library list, the open profile, and edit/save — built once.

## 4. Community fetch URL fix (cross-reference)

The library's "open community profile" path surfaced `Community fetch failed:
HTTP 400` for index entries whose `Spreadsheet URL` is a bare `…/d/{KEY}/edit`
(no gid). The transform in `yoke-index` was appending `&gid=0`, which 400s for
sheets whose first tab isn't gid 0. Corrected to **omit** gid when none is
known; see [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md) § 5.3
for the updated transform table and rationale.

## 5. Bindings panel — narrow-width layout

Binding triggers / modifiers / outputs are debug-formatted (`{:?}`) and can be
long (`System(DecrementMouseSpeed)`, `Side { dir: Puff, kind: Hard }`). The
fixed five-column row grid overflowed horizontally on a narrow panel, producing
a horizontal scrollbar and clipped output.

Layout rules:
- The flexible columns are `minmax(0, 1fr)` (trigger, output) and
  `minmax(0, auto)` (modifier), with `min-width: 0` on row children and
  `overflow-wrap: anywhere`, so long text wraps within the panel instead of
  overflowing. `.qs-bindings` is `overflow-x: hidden`.
- `.qs-bindings` is a `container-type: inline-size` query container. Below
  340 px the row switches from grid to `display: flex; flex-wrap: wrap`, so the
  parts wrap onto their own lines rather than the `1fr` columns collapsing to
  zero. Above 340 px the aligned grid is kept.

Container queries are baseline in the WKWebView/Blink versions Yoke targets.

## 6. Community catalog loading state

On first run the community index cache is empty, so `list_community_profiles`
makes a network round trip to `COMMUNITY_INDEX_URL`. The window, rail, and the
rest of the library paint immediately; only the **Community** section waits on
that fetch. Its `Show` fallback was a static `"Loading…"` string — no motion, so
it reads as a frozen app rather than work in progress. Subsequent runs hit the
warm cache and return instantly, which is why the stall is first-run-only.

Worse than slow: `community_profiles` was a plain `Vec<CommunityEntry>`, so an
empty vec meant *three* different things — not-yet-loaded, loaded-but-empty, and
failed. `spawn_community_fetch` swallowed the error arm (`if let Ok(entries) =
…`), so a slow **or failed** fetch left the section on `"Loading…"`
indefinitely. That is a genuine permanent hang, not just a slow one. A spinner
alone would inherit the same bug, so the fix models the load state.

Fix:

- **State (`state.rs`).** Replace `community_profiles: RwSignal<Vec<…>>` with
  `community: RwSignal<CommunityLoad>`, where
  `CommunityLoad { Loading, Loaded(Vec<CommunityEntry>), Failed(String) }`,
  initialized to `Loading`. The three states are now distinct.
- **Effect (`effects.rs`).** `spawn_community_fetch` stops swallowing errors:
  `Ok → Loaded(entries)`, `Err(e) → { tracing::error!(…); Failed(e.to_string()) }`.
- **Spinner (`components/spinner.rs`, new).** A reusable CSS-only ring,
  `<span class="qs-spinner" role="status" aria-label="Loading">`. No JS, no deps.
- **CSS (`components.css`).** `.qs-spinner` (ring with `--line` track, `--accent`
  head) + `@keyframes qs-spin`, and a `.qs-loading` flex row pairing the spinner
  with its label. The animation lives under a `@media (prefers-reduced-motion:
  reduce)` guard that renders the ring static — consistent with §1's native-feel
  stance.
- **Library (`library.rs`).** `CommunityProfileList` matches on `community`:
  `Loading` → spinner + "Loading community profiles…"; `Loaded([])` → "No
  community profiles."; `Loaded(entries)` → the existing list; `Failed(_)` →
  "Couldn't load community profiles."

**Scope boundary.** No retry button: a failed first-run fetch stays failed until
the app restarts. That is acceptable for v1 and strictly better than today's
infinite spinner; a retry affordance (and the same treatment for the
device-profile and open-community waits) is deferred. Only the Community section
changes — device and file sections render synchronously and never showed a
stall.

**Mock affordance.** `MockBackend::list_community_profiles` returns
synchronously, so the `Loading` state would flash for a single frame and the
spinner would never be seen under `trunk serve`. The mock gains a short
(~1 s) `gloo_timers` delay before returning so the spinner is observable in
standalone runs. The `Failed` arm is exercised by temporarily returning `Err`
from the mock, or by a real Tauri build with networking off.

## Verification

- Native feel, toggle-deselect, and Escape navigation exercised via `trunk
  serve` + `MockBackend` (browser): `user-select` computed `none`; station click
  filters then a second click / Escape clears to `ALL`; a second Escape returns
  to the library; the Escape listener does not leak across editor remounts.
- Context-menu suppression is release-gated and therefore not exercised by the
  debug `trunk serve` build; verified by inspection.
- URL transform: unit tests in `yoke-index` cover the no-gid, query-gid, and
  fragment-gid cases; the no-gid export was confirmed to return HTTP 200 live.
- Community loading state: `trunk serve` + `MockBackend` (with the ~1 s delay)
  shows the animated spinner, then the catalog list; `prefers-reduced-motion`
  renders the ring static. `Loaded([])` shows "No community profiles." and
  `Failed` shows the error line (mock temporarily returning `Err`), neither
  stuck on the spinner.
