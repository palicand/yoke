# Yoke GUI — custom window chrome

**Date:** 2026-07-18
**Stage:** F (follow-up)
**Status:** proposed
**Predecessors:** [`2026-06-13-yoke-gui-design-reconciliation-design.md`](2026-06-13-yoke-gui-design-reconciliation-design.md), [`2026-07-18-yoke-gui-deslop-design.md`](2026-07-18-yoke-gui-deslop-design.md)

## Goal

Replace the default OS window chrome with the design's `.win-chrome` strip.
This **reverses** the reconciliation spec's "no faux window chrome" decision,
deliberately: the default Windows title bar clashes badly with the app
(the original decision was made looking only at macOS), and the design mock
always drew its own chrome. The strip becomes the app's real title bar.

## The strip (all targets)

One 40px top strip replacing the current top panel (serif "Yoke" wordmark +
status text), per design `.win-chrome`:

- `bg-3` fill, 1px `line` bottom hairline.
- Centered window title "Yoke", 13px, semibold, `ink-1`.
- Right: connection pill (design `.conn`) — mono 11px `ink-2` text in a
  `bg-2` fill, `line` border, fully-rounded pill; leading 7px dot, `ac-green`
  when a device state is active, `ink-3` when not. Reuses the existing
  status-pill semantics, restyled.
- wasm: same visuals, no window commands (a browser tab has no chrome to
  manage).

## Per-OS window setup (`main.rs`)

- **macOS** — keep the native traffic lights, hide everything else:
  [`with_titlebar_transparent(true)`][vb] + [`with_fullsize_content_view(true)`][vb]
  + [`with_title_shown(false)`][vb]. Content extends under the (transparent)
  titlebar; the strip reserves a ~78px left inset so nothing collides with
  the lights. Native drag, zoom, minimize, fullscreen all keep working; the
  strip additionally arms [`ViewportCommand::StartDrag`][vc] on drag-start
  and toggles `ViewportCommand::Maximized` on double-click so the whole
  40px strip behaves like a titlebar, not just the native ~28px band.
- **Windows** — [`with_decorations(false)`][vb]; the strip draws caption
  controls right-aligned (minimize, maximize/restore, close) as
  painter-drawn glyphs (line / rect outline / cross — no icon font, no
  emoji), 46x40px hit targets, hover `bg-4`, close hover `#C42B1C` red.
  Buttons send `ViewportCommand::Minimized(true)` /
  `Maximized(!maximized)` / `Close`. Strip drag + double-click as on macOS.
  Edge resize: 4px hit zones along the window borders send
  [`ViewportCommand::BeginResize`][vc] with the matching `ResizeDirection`
  (undecorated winit windows have no native resize borders on Windows).
  Known, accepted losses: Win11 snap-layout flyout on the maximize button
  and the system window menu.
- **Linux** is not shipped; it takes the Windows path if ever built.

Platform branching uses `cfg!(target_os = ...)` runtime checks wherever the
code is portable (everything except the `ViewportBuilder` calls), so all
branches typecheck on every host — the Windows path cannot rot invisibly on
a macOS dev machine.

`ViewportCommand::Close` feeds the existing `close_requested` /
discard-changes flow unchanged.

## Testing

- Full gate set (fmt, clippy `-D warnings`, tests, wasm build).
- Manual on macOS: traffic lights, drag, double-click zoom, no title text.
- Windows behavior is compile-verified only (`ViewportCommand` is a
  cross-platform egui type); functional verification deferred to the first
  Windows run.

## References

- Design `.win-chrome` / `.conn`: claude.ai/design project
  `019e12c0-feeb-70c0-b52b-6b693d575e66`, `QuadStick App.html`.
- egui custom-frame pattern: [eframe `custom_window_frame` example][cwf].

[vb]: https://docs.rs/egui/0.34/egui/viewport/struct.ViewportBuilder.html
[vc]: https://docs.rs/egui/0.34/egui/viewport/enum.ViewportCommand.html
[cwf]: https://github.com/emilk/egui/blob/main/examples/custom_window_frame/src/main.rs
