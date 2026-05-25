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

## `yokectl` — command-line interface

`yokectl` is the scriptable surface for the same configuration model the
desktop app drives. It targets either a real QuadStick volume or a
filesystem-backed fake (`--fake-volume <dir>`), and emits human or JSON
output via `--json`. Build with `cargo build -p yokectl`.

Typical flows:

```sh
yokectl device                              # state of the attached volume
yokectl watch --json                        # stream mount events as NDJSON
yokectl list                                # profiles on the volume
yokectl show destiny                        # parsed view of one profile
yokectl validate ./local.csv                # parse + lint a local file
yokectl push ./local.csv destiny            # write a local file to the volume
yokectl pull destiny ./out.csv              # copy a volume profile out
yokectl install "Destiny 2"                 # fetch by community-index name
yokectl install https://docs.google.com/... # fetch by URL (CSV/Sheets)
yokectl install ./profile.csv               # install a local file
yokectl index list                          # browse the community index
yokectl set-binding destiny "sip hard" "left mouse"
yokectl apply ops.json                      # atomic batch edit
yokectl completions fish                    # shell completion script
yokectl docs --format md --out ./docs       # generate reference docs
yokectl topic install-sources               # in-binary topic page
```

`yokectl manual` opens the upstream QuadStick user manual in a browser;
`yokectl catalog` enumerates the inputs, outputs, preferences, modes, and
channels the binder understands. `yokectl --help` lists every subcommand
and `yokectl <cmd> --help` documents flags. The `topic` pages
(`install-sources`, `binding-model`, `preferences`, `sip-puff`,
`sub-profiles`) cover the concepts the flags assume.

## Credits

Yoke would not exist without Fred Davidson and the QuadStick project. The upstream Windows manager (QMP-4) is the authoritative reference for the device wire protocol used here.
