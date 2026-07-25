# Yoke GUI — design de-slop pass

**Date:** 2026-07-18
**Stage:** F (follow-up)
**Status:** proposed
**Predecessor:** [`2026-06-13-yoke-gui-design-reconciliation-design.md`](2026-06-13-yoke-gui-design-reconciliation-design.md)

## Goal

Apply the July 2026 "de-slop" pass of the design handoff to the shipped egui
GUI. The design source of truth is the claude.ai/design project
(`QuadStick App.html` + `src/*.jsx`, root-level files); the
`design_handoff_quadstick_config/` copy in the parent workspace is the
**pre**-de-slop revision and stays frozen. Because that project is not
reachable from the repo, every delta is enumerated here — this spec is
self-contained.

The de-slop direction: one restrained accent, neutral tags instead of colored
pills, no decorative glyphs, quieter defaults. Ornament is removed; category
color survives only on output glyphs.

## Token changes (`theme.rs`)

Console theme only (Studio/Contrast stay deferred, as before). oklch→sRGB
conversion cross-checked against the existing constants (old values reproduce
`0x6ED274`/`0x1E3B1F` exactly).

| Token | Old | New |
|---|---|---|
| `--accent` | `oklch(0.78 0.16 145)` = `#6ED274` | `oklch(0.76 0.11 145)` = `#84C485` |
| `--accent-2` | `oklch(0.32 0.06 145)` = `#1E3B1F` | `oklch(0.30 0.04 145)` = `#213321` |

All other console tokens (`--bg-*`, `--ink-*`, `--line*`, `--ac-*` category
hues, `--bg-binding`) are unchanged. The design's body-background radial
gradients were removed ("flat backdrop") — the GUI never painted them, so
nothing to do.

## Primitive changes (`theme.rs`)

- **`primary_button`** — design `.btn-primary` is now ink-colored, not accent:
  `ink_1` fill, `bg_1` text, `ink_1` border. (Used by editor "Save to
  QuadStick" and picker "Apply".)
- **`index_badge`** — deleted. The design dropped the `L1`/`L2` sub-tab index
  badges (`.sub-tab-i` is gone).
- **`kind_badge`** — de-colored: neutral `bg_3` fill, `ink_2` mono text, no
  color parameter (design `.kind-tag` lost its `.gamepad`/`.mk` colored
  variants). `ProfileKind`→color mapping in `library.rs` dies with it.
- **`category_tag`** — deleted. Category pills are gone from binding rows
  (`.evt-cat-pill` no longer rendered) and from picker rows
  (`.picker-cat-pill` no longer rendered). Category color lives only on
  output glyphs.
- **`glyph_box`** — deleted. Binding rows lost the leading short-code box
  (`.evt-short` is not part of the new `.brow`), and picker rows render the
  glyph as plain colored mono text (`.picker-glyph`), not a box.
- **`output_button`** — de-colored per new `.brow-out.set`: `bg_binding`
  fill, `line` border (solid), glyph keeps its category color, label is
  `ink_1`, no trailing category tag. Empty state ("+ Bind output") keeps a
  faint border + `ink_3` text (egui has no dashed strokes; solid-faint is the
  accepted approximation).
- **`mod_pill` quiet state** — new: a `normal` modifier renders at ~40%
  opacity with no border/fill, rising to ~80% with border while the pointer
  is over its row (design `.mod-pill.quiet`). Non-`normal` modifiers keep the
  current pill.
- **`eyebrow`, `kbd_hint`, `card_frame`, `row_frame`, `strip_frame`,
  `status_bar_frame`, `sub_tab_frame`, `segmented`, `clickable_frame`** —
  unchanged.

## Per-view changes

### Library (`views/library.rs`)

- Header: drop the "Profile Library" eyebrow; title becomes "Profiles"
  (serif italic, ~32px, down from 40); subtitle "N profiles · <mount state>".
- Toolbar: drop the "Sorted by name" hint.
- Cards (`.prof-card`): drop the top row (colored kind tag + mono filename).
  New layout: name, then footer = neutral kind tag left + "N layers" (mono,
  muted) right. Mode chips (`.sub-chip`) are gone. Filenames stay visible
  nowhere on the card — the name is the identity; the file name still shows
  in the editor breadcrumb and status contexts.
- The "On QuadStick" / "Community" section eyebrows stay — the mock has no
  community section; ours is real information architecture, not ornament.

### Editor (`views/editor.rs`)

- Title block eyebrow: variant name only (drop "· editing profile").
- Serif title ~24px (down from 28).
- Stat row: keep "N bindings"; drop the "M sub-profiles" stat. **Divergence:**
  the amber "unsaved" stat stays — the mock removed it because its dirty flag
  was fake; ours is real editor state.
- Sub-profile strip: drop the "SUB-PROFILE" label, drop `L{n}` index badges,
  drop the mode sublabel inside chips (chips = name + count only), drop the
  `increment_mode`/`decrement_mode` shift-hint if present. "Add layer" chip
  is plain quiet text (no dashed border). Rename/clone/delete affordances are
  preserved (real editor features).
- Full-width "＋" glyphs become ASCII "+" wherever they appear.

### Binding rows (`views/bindings.rs`)

- Drop the pane header's kind eyebrow ("ALL" / station kind — design
  `.bind-kind` is gone); the header is title + subtitle.
- Drop the leading glyph box and the "WHEN" eyebrow; the row starts with the
  input label (`.brow-when` holds only `.when-input` now).
- Modifier pill: quiet state for `normal` (see primitives).
- Output button: neutral `bg_binding` style, no category tag (see
  primitives).
- Empty-state and "+ Add binding" labels use ASCII "+".

### Device map (`views/map.rs`)

- Drop the "QS · FPS · INPUT MAP" title block from the painted map.
- `dev-meta` left label becomes "Click an input to filter bindings" (drop the
  variant name from it; the variant already shows in the editor eyebrow).
- Legend chips: drop the kind glyph (label + count only).

### Action picker (`views/picker.rs`)

- Rows: glyph as plain colored mono text (no box), label + csv desc; no
  category pill, no "↵" arrow.
- Footer: just "N outputs" / "N modifiers" — drop the "↑↓ navigate ↵ select"
  kbd hints (that navigation was never implemented; the hint advertised
  vapor).
- Key-capture banner: no leading "⌨" icon (if rendered).
- The search field's `esc` kbd hint stays.

### App shell (`app.rs`)

- Status bar: left = sync dot + state + mount path (one phrase, e.g.
  "/Volumes/QuadStick · in sync"); right = app version
  (`env!("CARGO_PKG_VERSION")`, mono, muted). Drop the device/community
  count clutter.
- Rail: no changes beyond what falls out of shared primitives (the rail
  never had icons).

## Explicitly unchanged / still deferred

- Studio + Contrast themes, Preferences screen, first-run DevicePicker,
  Diagram/Grid sketch variants, tweaks panel — deferred before, deferred
  still.
- Real OS title bar (no faux chrome) — standing decision.
- Fonts: Manrope UI / JetBrains Mono data / Instrument Serif display already
  match the de-slopped defaults (`font: sans`). No font work.

## Testing

- `theme.rs` unit test asserting the new accent constants.
- Existing pure-helper tests keep passing; view rendering stays untested
  (egui), as before.
- Full gate set: `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, wasm builds for
  `yoke-config` + `yoke-gui`.
- Visual pass against the design via `trunk serve` (wasm mock build).

## References

- Design: claude.ai/design project `019e12c0-feeb-70c0-b52b-6b693d575e66`,
  root `QuadStick App.html` / `src/app.jsx` / `src/device.jsx` /
  `src/picker.jsx` and its `HANDOFF.md` ("Design system (after de-slop
  pass, July 2026)").
- Pre-de-slop reference (frozen, do not modify):
  `design_handoff_quadstick_config/` in the parent workspace.
