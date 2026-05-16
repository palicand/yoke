# Yoke

Configuration software for the [QuadStick](https://www.quadstick.com/) — a sip-and-puff adaptive controller for quadriplegic gamers. macOS first; Windows planned. Configuration only — no firmware flashing.

## Why "Yoke"

A yoke is the control surface a pilot uses to translate small, precise human input into large, deliberate machine motion. That is exactly what a QuadStick does: it turns sips, puffs, lip pressure, and joystick motion into keystrokes, mouse moves, and gamepad signals. The hardware is still called QuadStick; Yoke is the software you configure it with.

## Status

Alpha. macOS only at this stage. Live device push is not yet implemented — the first usable version will save profiles to the FAT volume the QuadStick exposes when its mass-storage interface is enabled. See `docs/protocol/quadstick-wire-protocol.md` for the device-side details.

## Install and run

Not packaged yet. To work on Yoke locally:

1. **macOS prerequisites:** install Xcode Command Line Tools. `xcode-select --install`
2. **Quickest path (Nix):** install Nix with flakes enabled, then in this directory: `direnv allow` and `cargo tauri dev` (once Tauri crate added).
3. **Non-Nix path:** install rustup, then `cargo install trunk tauri-cli`

The full contributor flow, repo map, and house rules live in [`AGENTS.md`](./AGENTS.md).

## Credits

Yoke would not exist without Fred Davidson and the QuadStick project. The upstream Windows manager (QMP-4) is the authoritative reference for the device wire protocol used here.
