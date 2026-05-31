# Yoke scaffold and wire-protocol document — design

- **Date:** 2026-05-16
- **Status:** Approved, ready for implementation plan
- **Sub-project ID:** A (foundation)

## Context

Yoke is a native, all-Rust replacement for the QuadStick Manager Program (QMP).
It targets macOS first, with Windows planned. The eventual shape is a Tauri 2
desktop application built on a Leptos WASM frontend, plus a CLI binary for
programmatic and agent-driven workflows. The configuration substrate is the
FAT volume the QuadStick exposes when its mass-storage interface is enabled;
the secondary substrate is a wire protocol over HID feature/output reports and
a serial line, derived from the QMP-mac Python sources.

This document covers the **first sub-project only**: the repository scaffold
and a written wire-protocol document. No application code, no CSV parser, no
UI, no device communication — those are subsequent specs in their own files
under `docs/superpowers/specs/`.

## Goals

1. A single canonical place — this repository at `yoke/` — for the
   architectural decisions, the protocol knowledge harvested from the QMP-mac
   sources, and the contributor/agent conventions.
2. A reproducible development environment via Nix flake + direnv, so any
   contributor (human or agent) drops into the same toolchain.
3. A documentation skeleton that future sub-project specs slot into without
   rearranging.
4. An honest wire-protocol document that distinguishes what we know from what
   we are guessing, so the next reverse-engineering session has a clear list
   of open questions.

## Non-goals

- No Cargo crates created yet. Crates land in their own sub-projects, when the
  code that fills them is about to be written.
- No CSV parser, no profile model, no vocabulary catalog port.
- No Tauri or Leptos source code beyond what the eventual crate-creation
  sub-projects will introduce.
- No HID, serial, or volume code.
- No Windows-specific scaffolding beyond a placeholder in the flake.
- No firmware flashing surface, ever.

## Design

### 1. Workspace philosophy: lazy crate creation

The repository is a Cargo workspace, but the workspace's `members` array is
empty at the end of this sub-project. Crates are created one at a time when
the sub-project that needs them lands. This avoids speculative scaffolding,
keeps `cargo check --workspace` honest, and lets each crate's structure be
informed by the work it has to do rather than by a guess made up front.

The **intended eventual layout** below is a guideline for future
sub-projects, not a directive for this one. It is recorded here so subsequent
specs can reference it instead of re-deriving it.

| Crate | Kind | Targets | Purpose |
|---|---|---|---|
| `yoke-config` | lib | host + `wasm32-unknown-unknown` | Profile/prefs CSV serde, vocabulary catalog ported from `design_handoff_quadstick_config/src/data.js`, profile data model. Zero I/O. No platform code. Shared between backend and frontend. |
| `yoke-volume` | lib | host | Mount discovery and read/write on the QuadStick FAT volume. Behind a `VolumeProvider` trait; first impl is macOS via DiskArbitration, a filesystem-backed impl serves tests. |
| `yoke-device` | lib | host | Future home of the HID (Usage Page 0xFF00) and serial transports. Stays a stub with documented traits until protocol reverse-engineering reaches a usable state. |
| `yokectl` | bin | host | Command-line surface for listing, inspecting, validating, and editing profiles. Built on `clap`. |
| `yoke-tauri` | bin | host | Tauri 2 host process. Bridges Leptos IPC calls to `yoke-config` / `yoke-volume` / `yoke-device`. |
| `yoke-ui` | bin | `wasm32-unknown-unknown` | Leptos frontend. Imports `yoke-config` so IPC is type-safe. Must be runnable as a standalone browser app via `trunk serve` against a mock IPC backend; the Tauri shell is only the production wrapper. |

Two reasons the WASM-vs-native split forces `yoke-ui` and `yoke-tauri` to be
separate crates: Tauri ships them this way in its Leptos template, and
mixing target-specific dependencies in a single crate creates a tangle of
`cfg` gates that we do not need.

### 2. Build targets per crate (reference)

| Crate | Targets | Built by |
|---|---|---|
| `yoke-config` | host + `wasm32-unknown-unknown` | `cargo build` (host); `trunk` pulls it in as a dependency when building `yoke-ui` |
| `yoke-volume` | host only | `cargo build` |
| `yoke-device` | host only | `cargo build` |
| `yokectl` | host native binary | `cargo build -p yokectl` |
| `yoke-tauri` | host native binary; embeds the WASM artifact produced by trunk | `cargo tauri build` / `cargo tauri dev` |
| `yoke-ui` | `wasm32-unknown-unknown` | `trunk build` / `trunk serve` |

The shared-types story is the only reason `yoke-config` is dual-target:
backend and frontend agree on serialized profile types without duplicated
DTOs.

### 3. Repository layout at the end of this sub-project

```
yoke/
├── Cargo.toml              # virtual workspace, members = []
├── rust-toolchain.toml     # single source of truth for Rust + targets
├── flake.nix               # Nix devShell, reads rust-toolchain.toml via fenix
├── flake.lock              # committed
├── .envrc                  # `use flake`
├── README.md               # user-facing (see § 5)
├── AGENTS.md               # agent-facing conventions (see § 6)
├── LICENSE                 # already present
├── .gitignore              # Rust + Tauri + trunk + Nix-aware
├── .github/
│   └── workflows/
│       └── ci.yml          # runs inside the flake devShell
└── docs/
    ├── README.md           # docs index
    └── superpowers/
        └── specs/
            └── 2026-05-16-yoke-scaffold-design.md   # this file
```

No `crates/` directory yet. Created by the first sub-project that needs it.

**Plans are not committed.** `docs/superpowers/plans/` exists only as a
local working directory during sub-project execution, and is git-ignored.
The reasoning: a plan is an ephemeral artifact attached to one execution
pass — once the implementation lands, the spec carries the decisions and
the code carries the result; the plan adds no archival value and ages
out of relevance quickly.

**The wire-protocol document is not committed.** Originally drafted in
this sub-project at `docs/protocol/quadstick-wire-protocol.md`, the doc
lives instead in the maintainer's local Obsidian vault while many facts
are still `inferred` or `unknown`. Committing speculative-but-
authoritative-looking content to the repo would make guesses look like
ground truth. Once each fact reaches `confirmed (…)` status, the
relevant section is promoted back into the repo (likely as
`crates/yoke-device/PROTOCOL.md`). `docs/protocol/` is git-ignored.

### 4. Toolchain: Nix flake + direnv + rust-toolchain.toml

Two consumers, one source of truth.

`rust-toolchain.toml` is the single source of truth for the Rust toolchain
and targets:

```toml
[toolchain]
channel    = "1.96.0"
components = ["rustfmt", "clippy", "rust-analyzer"]
targets    = ["wasm32-unknown-unknown"]
```

The channel is pinned to an explicit version, not a rolling alias like
`stable`: fenix fetches the channel manifest as a fixed-output derivation
with a pinned hash, and a rolling alias rewrites that manifest on every
upstream release — breaking the hash and the devShell. Bump the version
deliberately and substitute the new hash in `flake.nix`.

This file is committed. Two consumers read it without redundancy:

- **Non-Nix contributors** (rustup-based, future Windows port): rustup reads
  `rust-toolchain.toml` automatically on any cargo invocation and installs
  the pinned channel, components, and targets on demand. No manual
  `rustup target add` step needed.
- **Nix contributors**: `flake.nix` reads the same file via
  `fenix.lib.fromToolchainFile { file = ./rust-toolchain.toml; }` so the
  Nix devShell uses the exact same toolchain spec. There is one source of
  truth and the two cannot drift apart.

The Nix `devShell` also provides, on top of the toolchain:

- `trunk` — WASM bundler for Leptos.
- `cargo-tauri` — Tauri 2 CLI.
- `pkg-config` — some Tauri transitive deps look for it.

**Platform prerequisites** (apply to Nix and non-Nix flows alike):

- **macOS**: Xcode Command Line Tools (`xcode-select --install`). Required
  for the linker, system headers, and the WebKit framework Tauri's webview
  uses. The flake does not install these because they are an Apple-licensed
  bundle outside nixpkgs.
- **Linux** (future): `webkit2gtk-4.1` and `libayatana-appindicator`. A
  commented-out block in `flake.nix` is ready to enable.
- **Windows** (future): WebView2 runtime (ships with current Windows 11);
  Visual Studio Build Tools. Not a Nix target.

`.envrc` contains `use flake` so direnv enters the devShell automatically on
`cd`. Non-Nix contributors skip direnv entirely and rely on rustup's
`rust-toolchain.toml` handling plus a one-time
`cargo install trunk tauri-cli` documented in AGENTS.md.

### 5. README.md (user-facing) — required content

The README is what someone visiting `github.com/palicand/yoke` sees. It must
include:

1. **What Yoke is.** One sentence: "Yoke is configuration software for the
   QuadStick." Plus a second sentence framing scope: macOS first, Windows
   planned, configuration only (no firmware flashing).
2. **The naming paragraph.** Two to three sentences distilled from
   `design_handoff_quadstick_config/NAMING.md` explaining why "Yoke": a yoke
   is the control surface that translates small precise human input into
   large machine motion, which is what a QuadStick does. The hardware is
   still called QuadStick; Yoke is the configuration software.
3. **Status banner.** Alpha; macOS only at this stage; live device push not
   yet implemented (v1 will save profiles to the mounted FAT volume).
4. **Install / run.** "Not packaged yet." Prereqs note for macOS: Xcode
   Command Line Tools via `xcode-select --install`. Quickest path:
   `direnv allow` to enter the Nix devShell, then `cargo tauri dev` once
   the Tauri crate exists. Non-Nix path: install rustup (the committed
   `rust-toolchain.toml` does the rest) plus `cargo install trunk
   tauri-cli`. Pointer to AGENTS.md for the full contributor flow.
5. **Credits.** Fred Davidson and the QuadStick project.

The README does **not** reference the Obsidian vault — the vault is local
context for the maintainer, not a useful pointer for external visitors.

### 6. AGENTS.md — required content

The AGENTS.md is read by every agent before working in the repo. It must
include:

1. **Repository map.** Where things live, what each crate is for (once they
   exist), where specs and plans go.
2. **Build, run, test commands** keyed by crate. Plus the rule that all
   commands run inside the flake devShell (direnv handles this
   automatically).
3. **House rules:**
   - No emojis in code, comments, docs, commit messages, PR bodies, or chat
     unless the user has used one in the current session or explicitly
     asked for one. This is a hard rule.
   - No comments except WHY-comments. Never explain WHAT the code does
     (well-named identifiers do that). Never reference the current task or
     fix in a code comment (that belongs in the PR description).
   - `thiserror` in library crates; `anyhow` in binary crates.
   - `tracing` for logging.
   - Dependencies added and removed only via `cargo add` / `cargo remove`,
     never by hand-editing `Cargo.toml` (workspace metadata edits excepted).
4. **Spec / plan workflow:**
   - Brainstorming output → `docs/superpowers/specs/YYYY-MM-DD-*.md`,
     **committed**. Specs are the authoritative architectural record.
   - Implementation plan → `docs/superpowers/plans/YYYY-MM-DD-*.md`,
     **local-only working artifact, not committed**. The
     `docs/superpowers/plans/` directory is git-ignored.
   - Implementation follows.
5. **Parallel-agent coordination.** When a task spans multiple crates or
   contains independent sub-tasks, use the host platform's parallel-agent
   primitive instead of serializing the work through a single agent.
   Independent crates typically warrant independent agents.
   Examples by platform:
   - Claude Code: `TeamCreate` for coordination, `Agent` with the
     `Explore` subagent type for read-only investigations.
   - Other platforms: equivalent multi-agent or parallel-task call.
   If your platform has no such primitive, fall back to sequential work
   and note it in the PR description.
6. **UI development rule.** `yoke-ui` must remain runnable as a standalone
   browser app via `trunk serve` against a mock IPC backend, in addition to
   running inside Tauri. This is what lets non-local agents iterate on the
   UI through a regular browser; the Tauri shell is the production wrapper,
   not the development substrate.
7. **Fixtures.** Example QuadStick config CSVs live at
   `../examples/` in the parent workspace directory; copies for unit tests
   land under `fixtures/csv/` when `yoke-config` is created. The mounted
   QuadStick volume may or may not be present and its mount is racy on
   macOS; underlying USB/HID details live in the maintainer's local
   wire-protocol notes (not committed — see § 7 of this spec for why).
8. **Non-Nix contributors.** macOS: `xcode-select --install`, then
   install rustup (`rust-toolchain.toml` auto-fetches the channel,
   components, and `wasm32-unknown-unknown` target on first `cargo`
   invocation), then `cargo install trunk tauri-cli`. Windows
   instructions land when the Windows port begins.
9. **Platform prerequisites.** macOS: Xcode Command Line Tools. Linux
   (future): `webkit2gtk-4.1`, `libayatana-appindicator`. Windows
   (future): WebView2 runtime, Visual Studio Build Tools. The flake
   covers these on Linux but not on macOS or Windows.

### 7. Wire-protocol notes — kept out of git for now

The QuadStick wire-protocol notes are **not committed** to this repo.
They live in the maintainer's local Obsidian vault while many facts
are still `inferred` or `unknown` — committing speculative-but-
authoritative-looking content would make guesses look like ground
truth, and future contributors would treat them as decided.

`docs/protocol/` is git-ignored. The notes still get drafted (and the
content checklist below records what they must cover), but publication
to the repo is gated on confirmation.

**Promotion criterion.** A section is ready to move into the repo
(likely as `crates/yoke-device/PROTOCOL.md` or similar) when every
fact in it carries a `confirmed (…)` status tag — no `inferred` and
no `unknown` rows remain.

**Source repositories:**

- QMP-4 (upstream, Windows-complete): <https://github.com/fdavison/QMP-4>
  — Fred Davidson's canonical manager. Most feature-complete: includes
  ViGEmBus/HIDHide integration, voice/Vocola hooks, firmware flashing, and
  external-pointer surfaces. **Primary source.**
- QMP-mac (macOS fork of the above):
  <https://github.com/cchriskeach/QMP-4> — Chris Keach's fork that strips
  Windows-only behaviors for macOS support. **Secondary source**, used to
  identify which behaviors are platform-portable.

Every fact in the document cites whichever source it came from, and the
status column distinguishes confirmed-from-QMP-4 from
confirmed-only-in-QMP-mac. The prior hardware probe (`ioreg` dumps,
descriptor decodes) supplies the USB/HID layer.

The notes (wherever they live) must cover:

1. **Hardware identity.** USB IDs:
   - Primary: VID `0x16D0` ("MCS Electronics" shared pool), PID `0x092B`.
   - Emulation aliases visible to the host: PID `0x092C` (X360CE mode),
     `0x092D` and `0x092E` (the device exposes itself with different PIDs
     across PS3/PS4/Switch emulation modes).
   - Hori PS4 fallback identity: VID `0x0F0D`, PID `0x0066`. The device
     advertises itself as Hori in PS4 mode.
   - Legacy unit: VID `0x1fc9`, PID `0x205B`.
   Source: `QMP-4/QuadStick Manager Program/QuadStickHID.py` (cross-checked
   against the QMP-mac fork). Confirmed.
2. **Interface layout.** Three USB interfaces when the device is in M&K
   profile: Interface 0 is the gamepad HID plus the vendor-defined channel
   on Usage Page `0xFF00`; Interface 1 is a mouse HID; Interface 2, when
   present, is the FAT mass-storage interface. Whether the mass-storage
   interface enumerates depends on `enable_DS3_emulation` in `prefs.csv`.
   Source: `ioreg` probe from 2026-05-09; HID descriptor decode in
   `ref_quadstick_hardware.md`.
3. **Three transports.** The protocol can travel over three substrates that
   speak the same upper-layer command language:
   - **HID input report (64 B):** device → host telemetry stream.
     Continuous. Carries gamepad state, mouse motion, and (on Interface 0,
     usages `0x20`–`0x2F`) sip/puff sensor telemetry.
   - **HID feature report (8 B), Usage Page `0xFF00`, Usage `0x2621`:**
     bidirectional. Read by `IOHIDDeviceGetReport`, written by
     `IOHIDDeviceSetReport`. Used by QMP for external-pointer updates and
     similar low-volume signals. Source:
     `QuadStickHID.send_feature_report`.
   - **HID output report (8 B):** host → device commands.
     `QuadStickHID.sendline` (present in both QMP-4 and QMP-mac) wraps a
     string in `\r…\r`, pads to a multiple of 8 bytes, and emits
     successive 8-byte output reports. The payload semantics are the same
     as the serial transport — same command vocabulary, different
     framing.
   - **Serial port (115200 baud, 8N1):** the device exposes a USB-CDC
     serial endpoint that speaks the same text protocol. Probe: send
     `\rreset\r`, expect `all outputs reset` in the response. Source:
     `QMP-4/QuadStick Manager Program/microterm.py`.
4. **Line protocol.** Commands are ASCII strings terminated by `\r` (CR).
   The serial transport and the HID output transport carry the same
   commands. The only command confirmed from source is `reset` (response:
   `all outputs reset`). The full vocabulary must be derived from a
   `QMP-4/QuadStick.py` audit (search for callers of `sendline` and
   `send_external_pointer_update`; QMP-4 has the most complete set of
   callers including the firmware-flash and Vocola voice paths) and
   ultimately validated by capturing QMP-to-device traffic in a Windows
   VM with USBPcap.
5. **Gating flags.** From `prefs.csv` and per-config preference overrides:
   - `enable_usb_comm` must be `1` for the device to respond to the HID
     command channel. Current user's prefs has `0`. Almost certainly the
     reason the command channel currently appears inert.
   - `enable_DS3_emulation` controls whether the mass-storage interface
     enumerates. Modes `0`, `2`, and `4` expose it; other values typically
     hide it. Per-config CSVs can override the global preference.
   - PS4 / Hori mode breaks USB command access entirely; the device
     re-enumerates under the Hori VID/PID with a stripped HID descriptor.
     Yoke should detect this and present a clear "switch out of PS4 mode"
     instruction.
6. **Volume gotchas.** The FAT mass-storage interface enumerates racily on
   Apple Silicon (composite-device init race; see
   `ref_macos_usb_diagnostics.md`). HID input always works; the
   `/Volumes/Quad Stick` mount may or may not be present at any given time.
   Sleep/wake often kicks the mount into existence without a replug.
7. **Status table.** For every concrete fact in the document, a column
   indicating whether it is:
   - `confirmed (QMP-4)` — read directly from the upstream Windows source.
   - `confirmed (QMP-mac)` — present only in the macOS fork; flag as
     potentially incomplete relative to QMP-4.
   - `confirmed (hardware)` — observed on the actual device via `ioreg`,
     HID descriptor decode, or a runtime probe.
   - `inferred` — strongly implied by surrounding code or behavior but not
     directly visible.
   - `unknown` — needs a Wireshark/USBPcap capture in a Windows VM, or a
     reply from Fred Davidson.
8. **Open questions.** A bulleted list of the specific captures or source
   audits needed to upgrade `inferred` and `unknown` rows to `confirmed`.

The protocol notes are deliberately scoped to **transport mechanics and
known commands**, not the command vocabulary. The vocabulary itself is the
deliverable of the future device-comm sub-project, and will live in
`yoke-device` source plus its own spec.

### 8. CI

`.github/workflows/ci.yml` runs on push and pull request. It installs Nix
(via DeterminateSystems' nix-installer-action and magic-nix-cache-action),
enters the flake's devShell, and runs:

- `cargo metadata --no-deps --format-version 1` — workspace integrity
  gate; succeeds on an empty workspace.
- `cargo fmt --all -- --check` — gated by presence of
  `crates/**/Cargo.toml` (skipped while `members = []`).
- `cargo clippy --workspace --all-targets -- -D warnings` — same gate.
- `cargo check --workspace` — same gate.
- `cargo test --workspace` — same gate.

The conditional gating is necessary because `cargo` errors out on an
empty virtual workspace under current cargo, but `cargo metadata`
doesn't. The four full-fat cargo gates re-activate the moment the
first crate lands without a CI change.

### 9. Acceptance criteria

This sub-project is done when:

- `README.md`, `AGENTS.md`, `docs/README.md`, and this spec are
  committed to the repository. (The wire-protocol notes are NOT
  committed — see § 7.)
- `flake.nix`, `flake.lock`, `.envrc`, and `rust-toolchain.toml` exist;
  `direnv allow` succeeds; `nix develop --command true` exits 0; the
  flake's Rust toolchain is sourced from `rust-toolchain.toml` (verified
  by inspection of `flake.nix`).
- Inside the devShell:
  `rustc --print target-libdir --target wasm32-unknown-unknown` succeeds.
- Inside the devShell: `trunk --version` and `cargo tauri --version` both
  succeed.
- `cargo metadata --no-deps --format-version 1` succeeds with an empty
  `workspace_members` array.
- The CI workflow file exists and lint-checks as valid YAML.
- `.gitignore` covers Rust target dirs, Tauri build artifacts, trunk
  output, Nix `result` symlinks, editor scratch files,
  `docs/superpowers/plans/` (plans are local-only), and
  `docs/protocol/` (wire-protocol notes are local-only).

## Out of scope (queued for future sub-projects)

- **B — `yoke-config`:** port the vocabulary catalog from `data.js`;
  implement CSV parse/serialize against the example fixtures;
  round-trip every input, output, and modifier through unit tests.
- **C — `yoke-volume`:** macOS DiskArbitration backend, filesystem-backed
  test backend, mount/unmount lifecycle.
- **D — `yokectl`:** CLI surface backed by `yoke-config` and
  `yoke-volume`.
- **E — Tauri shell + UI v1 (read-only viewer):** `yoke-tauri` and
  `yoke-ui` minimal scaffolding, open a profile and render the
  design-handoff layout, no save.
- **F — UI v2 (editor):** binding edits, key-capture banner, save back.
- **G — `yoke-device`:** HID 0xFF00 transport, serial transport,
  command-vocabulary RE.
- **H — Windows port:** `yoke-volume` Windows backend; revisit `flake.nix`
  to mark Linux explicit; document Windows-native install path.
- **I — Live device push:** replace mounted-volume saves with the HID
  command channel once `yoke-device` reaches parity.

## Forward references

- `docs/protocol/quadstick-wire-protocol.md` — produced by this
  sub-project; consumed by sub-project G.
- The `design_handoff_quadstick_config/` directory in the parent workspace
  is the visual reference for sub-projects E and F. Treat it as design
  artifact only; the React/JSX implementation in that bundle is not a port
  target.
- `examples/*.csv` in the parent workspace is the test corpus for
  sub-project B. Copies will live under `fixtures/csv/` once that crate
  exists.
