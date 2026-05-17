# yoke-config — design

- **Date:** 2026-05-16
- **Status:** Approved, ready for implementation plan
- **Sub-project ID:** B (`yoke-config`)

## Context

The QuadStick is configured through CSV files written to a FAT volume the
device exposes when its mass-storage interface is enabled. A "configuration"
is one or more `*.csv` files: `default.csv` (always present, always loaded
first), zero or more game-specific configs (`destiny.csv`, `forza5.csv`, …),
and a singleton `prefs.csv` for device-wide preferences. Each config CSV
contains one or more **sub-profiles** (Mouse / Left Analog / Right Analog /
Mixed / D-Pad / etc.), and within each sub-profile a list of **bindings**
that connect inputs (sip/puff positions, lip switch, joystick directions,
USB-A host axes) to outputs (keyboard, mouse, gamepad buttons, console-
specific actions). Modifiers (`normal`, `toggle`, `delay_on`, `repeat`, …)
sit between input and output and shape the firing behavior.

The canonical authoring workflow is Google Sheets: the user copies one of
Fred Davidson's published templates, edits in Sheets, exports as CSV, and
drops the file on the QuadStick volume. The Google Sheets exporter is the
de-facto reference implementation for the on-disk format; the QuadStick
firmware is the only consumer with authority to refuse a file. There is no
formal spec — the closest thing is Fred's user manual at
<https://quadstick.s3.amazonaws.com/documents/user_manual/um/configuration.htm>
and its subpages.

This sub-project produces `yoke-config`: a pure-Rust library that parses
these CSVs into a strongly-typed model, serializes a model back into CSV
bytes the firmware accepts, and exposes the full vocabulary catalog
(inputs, outputs, modifiers, preferences, sub-profile modes, device
variants) as Rust constants. It is the foundation every other Yoke
sub-project sits on: `yoke-volume` (C) reads/writes profiles through it,
`yokectl` (D) and `yoke-ui` (E/F) consume its types.

## Goals

1. Parse every CSV produced by Fred's canonical Google Sheets templates
   without dropping data. Unrecognized identifiers survive as `Unknown(_)`
   enum variants so a future firmware revision cannot make us lose files.
2. Serialize back to bytes the QuadStick firmware accepts. The output is a
   strict subset of what Google Sheets' CSV exporter would emit: A–J = data,
   K+ = optional comments, RFC4180 quoting where needed, first blank
   Output row terminates the section (firmware-critical rule).
3. Provide a typed catalog so `yoke-ui` (Leptos / WASM) and `yoke-tauri`
   (host) share enums and don't drift on serialization.
4. Compile as both a host crate and a `wasm32-unknown-unknown` crate.
   Zero I/O, zero platform code, zero `std::fs`.

## Non-goals

- No file I/O, no path handling, no `/Volumes/Quad Stick` discovery.
  That is `yoke-volume` (sub-project C).
- No CLI, no UI, no IPC, no device commands.
- No firmware-side semantics beyond what the firmware refuses on disk
  (e.g. "is this binding combination physically meaningful"). The catalog
  documents the vocabulary; it does not validate semantic correctness of a
  user's choices.
- No live-mirroring of upstream catalog sources at build time or runtime.
  The catalog is hand-ported, static Rust source. It drifts when Fred
  updates the official template; we re-port on a refresh cadence we choose.
- No persistence of round-trip fixtures inside the repository (see § 6).
- No Infrared sub-profile editing semantics yet. The parser recognizes the
  `Infrared` section type and round-trips it, but the model treats its
  contents as opaque `RawSection` data until a future sub-project gives
  it a typed shape.

## Design

### 1. Architecture: two-layer (raw + semantic)

QuadStick CSVs are not strict RFC 4180. They are section-delimited (blank
row separates sections), positional within a section, with section-level
metadata in fixed cells of the first three rows. To honor lossless behavior
and still give downstream crates a clean typed model, the crate runs two
layers:

- **`raw`** — verbatim tokenization. `RawCsv { top_line, sections:
  Vec<RawSection> }` where `RawSection` is the cell grid for one
  sheet-section, preserving every cell exactly as Google Sheets emitted it
  including columns K and beyond.
- **`model`** — typed semantic view. `Profile { top_line, sub_profiles:
  Vec<SubProfile>, preferences: Option<Preferences>, infrared:
  Vec<RawSection> }` where each `SubProfile` holds `bindings:
  Vec<Binding>`, `overrides: Vec<PreferenceOverride>`, channel, mode, and
  the parsed-but-preserved section header.

Both directions exist as functions:

- `parse(bytes: &[u8]) -> Result<ParseResult, ParseError>`
- `write(profile: &Profile, template: Option<&RawCsv>) -> Result<Vec<u8>, WriteError>`

where `ParseResult { raw: RawCsv, model: Profile, warnings: Vec<Warning> }`.

The `template` arg to `write` is the lossless escape hatch: when the caller
has the original `RawCsv` (which `parse` already produced), `write` uses it
to reconstruct trailing-column counts, the original sub-profile mode label
(e.g. `Left joy` vs `Left Analog`), comment cells in column K+, and any
`Unknown(_)` payloads exactly. Without a template, `write` emits a canonical
form (documented identifiers, lowest-trailing-comma layout) that the firmware
accepts.

**Round-trip invariant** (test gate, not library contract):
`write(parse(bytes)?.model, Some(&raw)) == bytes` for every fixture in the
in-source test suite. Byte-identical round-trip is *not* guaranteed for
arbitrary user-supplied input, because Google Sheets' CSV quoting rules can
emit forms our writer doesn't reproduce exactly — but the parsed model is
always semantically intact.

### 2. Module layout

```text
crates/yoke-config/
├── Cargo.toml
└── src/
    ├── lib.rs                         # re-exports
    ├── catalog/                       # ports of upstream vocabulary
    │   ├── mod.rs
    │   ├── inputs.rs                  # MP positions, side tube, lip,
    │   │                              # joystick, D-pad inner/outer,
    │   │                              # USB-A host 1/2 axes & buttons,
    │   │                              # digital_in_1..8, center, constant
    │   ├── outputs.rs                 # kb_*, mouse_*, gamepad (PS + XB +
    │   │                              # Switch), dpad_*, left/right_joy_*,
    │   │                              # increment_mode, decrement_mode,
    │   │                              # touch
    │   ├── modifiers.rs               # normal, toggle, repeat, pulse,
    │   │                              # duty, greater_than, less_than,
    │   │                              # force_off, delayed_latch,
    │   │                              # delay_off, delay_on, tap,
    │   │                              # increment_value, decrement_value
    │   ├── subprofile_modes.rs        # canonical names + legacy synonyms
    │   ├── channels.rs                # USB, Bluetooth
    │   ├── preferences.rs             # PREF_GROUPS, defaults, valid
    │   │                              # ranges from manual + template
    │   └── variants.rs                # device topology (FPS, Singleton)
    ├── model/
    │   ├── mod.rs
    │   ├── profile.rs                 # Profile, SubProfile, TopLine
    │   ├── binding.rs                 # Binding, Input, Output enums
    │   ├── modifier.rs                # Modifier enum with typed args
    │   ├── preferences.rs             # Preferences (typed prefs map)
    │   └── overrides.rs               # PreferenceOverride (per-sub-profile)
    ├── csv/
    │   ├── mod.rs
    │   ├── raw.rs                     # RawCsv, RawSection, RawRow
    │   ├── parse.rs                   # bytes -> ParseResult
    │   └── write.rs                   # Profile (+template) -> bytes
    └── error.rs                       # ParseError, WriteError, Warning
```

Dependencies, added via `cargo add`:

- `csv` — tokenization. Handles RFC4180 quoting we'd otherwise re-derive.
- `serde` with `derive` — for the IPC boundary that `yoke-tauri` and
  `yoke-ui` will use later. Every public model type is `Serialize +
  Deserialize`.
- `thiserror` — error types per the workspace rule.

No `chrono`, no `std::fs`, no `tokio`, no platform-conditional deps. The
crate compiles unchanged for `wasm32-unknown-unknown`.

### 3. Type sketch (the load-bearing parts)

```rust
// model/binding.rs
pub enum Input {
  Mouthpiece { pos: MpPosition, dir: SipPuff, soft: bool, long: bool },
  Side { dir: SipPuff, soft: bool, long: bool },
  Lip { soft: bool },
  Joystick(JoyAxis),
  JoystickDpad(DPadDir),
  JoystickDpadInner(DPadDir),
  AnyDirection,
  Center,
  Constant,
  UsbHostAxis { host: UsbHost, axis: JoyAxis },
  UsbHostDpad { host: UsbHost, dir: DPadDir, inner: bool },
  UsbHostButton { host: UsbHost, button: u8 },
  DigitalIn(u8),
  Unknown(String),
}

pub enum Output {
  Keyboard(KbKey),
  Mouse(MouseAction),
  Gamepad(GamepadButton),
  Dpad(DPadDir),
  Joystick(JoyOutput),
  System(SystemAction),
  Touch,
  Unknown(String),
}

pub enum Modifier {
  Normal,
  Toggle,
  DelayOn { ms: Option<u32>, second: Option<u32> },
  DelayOff { ms: Option<u32> },
  GreaterThan { pct: Option<u8>, upper: Option<u8> },
  LessThan { pct: Option<u8> },
  Repeat { hz: Option<u32>, delay_ms: Option<u32> },
  Pulse { ms: Option<u32>, count: Option<u32> },
  Duty { ms: Option<u32> },
  ForceOff { ms: Option<u32> },
  DelayedLatch { ms: Option<u32> },
  Tap { window_ms: Option<u32>, pulse_ms: Option<u32> },
  IncrementValue { amount: Option<i32>, interval_ms: Option<u32> },
  DecrementValue { amount: Option<i32>, interval_ms: Option<u32> },
  Unknown { name: String, args: Vec<String> },
}

pub struct Binding {
  pub output: Output,
  pub modifier: Modifier,
  pub input: Option<Input>,
  pub comment: Option<String>,        // column K+, joined
}

pub struct PreferenceOverride {
  pub key: PreferenceKey,             // typed; falls back to PreferenceKey::Unknown(String)
  pub value: String,                  // typed coercion happens in PreferenceKey::parse
  pub comment: Option<String>,
}
```

`Input::input` on a `Binding` is `Option` because the canonical template
emits placeholder bindings with a blank input cell (the user has reserved
the row for an output they haven't bound yet). The blank-Output rule, by
contrast, terminates a section — that is a parser-level signal, not a model
state.

### 4. Catalog sourcing

Vocabulary is hand-ported, static Rust source. Three reference sources, in
priority order, reconciled by the implementer once:

1. **Fred's user manual** at
   <https://quadstick.s3.amazonaws.com/documents/user_manual/um/configuration.htm>
   and the linked dropdown subpages
   (`dropdown_list_for_outputs.htm`, `dropdown_list_for_inputs.htm`,
   `dropdown_list_for_ouput_functions.htm`, `preferences.htm`,
   `selecting_output_names_for_playstation_and_xbox.htm`,
   `usb_bluetooth_channel_selection.htm`,
   `changing_profiles.htm`). Authoritative for the documented vocabulary.
2. **Fred's Google Sheets template** at
   <https://docs.google.com/spreadsheets/d/1L9jM97fJHVxYmQF7eHIPMSWI9golxXEZjmFsyCaWvsA/edit>
   (the maintainer's editable copy of Fred's official template). Authoritative
   for what current configurations actually use, including identifiers the
   manual hasn't been updated to mention (e.g. `touch`).
3. **The canonical index of community configurations** at
   <https://docs.google.com/spreadsheets/d/e/2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/pubhtml?gid=1483029791&single=true>.
   Roughly 310 entries pointing at individual config sheets. Authoritative
   for identifiers that appear in the wild but not in the official manual
   or template (Windows-only emulation modes, UltraStik, BSC, Beloader,
   exotic accessory configs).

The implementer fetches these sources, manually reconciles, and writes
typed enums plus `const FOO_IDS: &[&str]` arrays into the `catalog/`
modules. The manual URL is recorded as a `// source:` annotation next to
each catalog table — the one WHY-comment exception that earns its keep,
because catalog drift is the most likely silent-break failure mode.

The catalog is **static**. We do not refetch on CI, on build, or at
runtime. When Fred ships an updated template, the maintainer re-ports.
Stale catalog manifests as `Unknown(_)` variants on user files; the
library still loads and saves the file correctly, only the UI affordance
for the missing identifier is unavailable. This is acceptable because:

- Catalog refreshes are rare in practice (low-rate firmware revisions).
- The `Unknown(_)` escape variant means staleness degrades the UI, not
  user data.
- Building a live-mirroring pipeline would be heavy infrastructure for a
  rare event, and would couple our build to upstream availability.

### 5. Sub-profile mode synonyms

Section headers in the wild use multiple names for the same logical sub-
profile mode. The catalog records both forms; the parser accepts either;
the writer emits whichever form the section originally had (preserved on
the `RawSection` template).

Observed pairs, derived from the manual, Fred's current template, and the
corpus:

| Canonical (current template) | Legacy (corpus, `data.js`) |
|---|---|
| `Mouse` | `Mouse Mode` |
| `Mouse Scroll` | (new — corpus uses bindings on `mouse_wheel_*`) |
| `Left Analog` | `Left joy` |
| `Right Analog` | `Right joy` |
| `Mixed Analog` | `Mixed Joystick` |
| `D-Pad` | `D-Pad` (same) |

The model uses a single `SubProfileMode` enum with these as canonical
discriminants; the parser maps both spellings to the same variant; the
writer's default canonicalizes to the current-template form unless a
template is supplied.

### 6. Test corpus — outside the repository

No QuadStick CSV files are committed to the repo. The user-authored
configurations on the canonical index are user content; mirroring them is
out of scope and a maintenance burden. The S3 manual is hosted elsewhere
and is cited by URL.

The test strategy is therefore:

- **Inline CSV strings in test files.** Each round-trip test embeds the
  CSV under test as a raw string literal (`r#"..."#`) inside the
  `#[test]` function. These are *test inputs the maintainer authored to
  exercise specific cases* — not mirrors of community configs. Specific
  cases to cover at minimum: a single-sub-profile file; a multi-sub-
  profile file with each mode synonym pair; a file with `delay_on 1000`
  vs `delay_on` (no arg); a file with a preference override row; a file
  with column-K comments; a file with an unknown output/input/modifier
  (exercises `Unknown(_)` round-trip); a `prefs.csv` standalone; a file
  with an `Infrared` section; a file with `Channel = Bluetooth`.
- **Optional dev-machine corpus.** If `YOKE_CORPUS_DIR` is set in the
  test environment, the test suite walks that directory and runs
  parse-then-write-then-parse against every `*.csv` it finds. CI does
  not set this. Local maintainers point it at their own copy of the
  canonical corpus when validating a catalog refresh. Tests skip
  silently when the env var is absent.
- **Catalog unit tests.** Every catalog constant id is round-trip-
  parseable (`Input::from_csv("mp_center_puff")?.to_csv() ==
  "mp_center_puff"`); every preference key parses to a typed value within
  its declared range.

### 7. Parsing rules (firmware-critical)

From the manual at
<https://quadstick.s3.amazonaws.com/documents/user_manual/um/google_drive_spreadsheets.htm>:

1. Columns A–J are configuration data. Columns K and beyond are comments
   and ignored by the firmware. The parser preserves K+ as
   `Binding.comment`; the writer emits K+ only when `comment.is_some()`.
2. Each section begins with three header rows:
   - Row 1: `<section type>, <profile name>, <sub-profile mode>, <…>`
     where section type is one of `Profile Name`, `Preferences`,
     `Infrared`.
   - Row 2: `, , <sub-mode label>, <…>` (the second word in the section
     identity — typically `Normal` for `Profile Name` sections).
   - Row 3: column-name headers, with column C holding the section
     channel (`USB` or `Bluetooth`).
3. The first row with a blank column-A value within a section terminates
   that section. Per the manual: "any contents following the first blank
   row can break the configuration file." The parser respects this; if
   non-blank content follows a blank-A row, it is captured into
   `ParseResult.warnings` as a `Warning::DataAfterTerminator`. The writer
   never emits such data.
4. Sections are separated by blank rows. Multiple blank rows between
   sections are preserved on the raw layer.
5. Within a binding row, the modifier cell is `<modifier> [<arg1>
   [<arg2>]]` — space-separated. The parser splits on whitespace and maps
   the first token to a `Modifier` variant, with remaining tokens parsed
   as the variant's args. Unrecognized modifiers become
   `Modifier::Unknown { name, args }` and round-trip verbatim.
6. A binding-row whose Output column matches a known `PreferenceKey` is
   parsed as a `PreferenceOverride` for that sub-profile, not a
   `Binding`. The input column then holds the value (e.g.
   `joystick_dead_zone_shape, normal, 1`).
7. Line endings: the parser accepts `\r\n`, `\n`, or `\r`. The writer
   emits `\r\n` to match Google Sheets' CSV export.
8. Encoding: UTF-8. The parser refuses non-UTF-8 input with
   `ParseError::Encoding`.

### 8. Errors and warnings

`thiserror` enums per the workspace rule.

- **`ParseError`** — hard failure. Variants: `Encoding`,
  `MissingTopLine`, `MalformedSectionHeader { line }`, `UnclosedQuote {
  line }`. Carries source-position context where applicable.
- **`Warning`** (non-fatal, collected in `ParseResult.warnings`):
  `UnknownOutput { id, line }`, `UnknownInput { id, line }`,
  `UnknownModifier { name, line }`, `UnknownPreference { id, line }`,
  `PreferenceOutOfRange { key, value, expected_range, line }`,
  `DataAfterTerminator { line, count }`, `DuplicateBinding { input,
  line }`. The UI surfaces these; the file still loads.
- **`WriteError`** — only fires on internal invariant violations, e.g.
  a `Profile` whose `SubProfileMode` cannot be encoded (impossible to
  construct from user input via `parse`; reachable only if a caller
  hand-builds an inconsistent model).

### 9. Serde + WASM compatibility

Every public model type derives `serde::Serialize` and `serde::Deserialize`
so that the eventual Tauri IPC boundary (sub-project E) can ferry
`Profile` and `Binding` between `yoke-tauri` and `yoke-ui` without
hand-written DTOs. The crate compiles for both `--target x86_64-apple-
darwin` and `--target wasm32-unknown-unknown`; CI gates on both.

There is no `wasm-bindgen`, no `js-sys`, no platform-conditional code in
this crate. The `wasm32` target is achieved by avoiding incompatible deps
rather than by configuring around them.

### 10. Acceptance criteria

This sub-project is done when:

- `crates/yoke-config/` exists, builds clean (`cargo build -p yoke-
  config`), and `cargo clippy -p yoke-config -- -D warnings` is clean on
  the host target.
- `cargo build -p yoke-config --target wasm32-unknown-unknown` is clean.
- `cargo test -p yoke-config` passes — covers catalog unit tests, the
  in-source inline-CSV round-trip suite, and exercises every variant of
  `ParseError`, `Warning`, and `WriteError`.
- The catalog's `Input`, `Output`, `Modifier`, `PreferenceKey`, and
  `SubProfileMode` enums each have a documented `// source: <url>`
  annotation citing the upstream page they were ported from.
- `parse` followed by `write(_, Some(&raw))` returns byte-identical
  output for every inline-CSV test fixture in the suite.
- The catalog covers every identifier observed in: the manual's
  dropdown subpages; Fred's current Google Sheets template (snapshot
  read during this sub-project); and at minimum the maintainer's
  pre-existing Mac corpus (`../examples/*.csv`). Coverage against the
  full community corpus is a maintainer-local validation step (run
  with `YOKE_CORPUS_DIR=…`); it is not required to be clean at merge
  time, but any gaps found get logged and addressed before merging.

## Out of scope (queued for future sub-projects)

- **C — `yoke-volume`:** macOS DiskArbitration backend, filesystem-backed
  test backend, mount/unmount lifecycle, profile-list discovery.
- **D — `yokectl`:** CLI surface built on `yoke-config` + `yoke-volume`.
- **E — Tauri shell + UI v1 (read-only viewer):** scaffolding for
  `yoke-tauri` and `yoke-ui`, render a parsed `Profile` against the
  design-handoff layout.
- **F — UI v2 (editor):** binding edits, modifier-arg editing, save back
  through `yoke-volume`, key-capture banner, action picker.
- **G — `yoke-device`:** HID 0xFF00 transport, serial transport, command-
  vocabulary RE.
- **H — Windows port:** `yoke-volume` Windows backend.
- **I — Live device push:** replace mounted-volume saves with the HID
  command channel from G.

Validation work explicitly deferred to its own sub-project:

- Semantic validation beyond catalog membership (e.g. "this Input is not
  meaningful in this `SubProfileMode`") happens in Stage F, when the
  editor surfaces it.
- Infrared sub-profile editing semantics happen no earlier than Stage F.
  Until then, Infrared sections round-trip as `RawSection` opaque data.

## Forward references

- The vocabulary catalog sources are URLs listed in § 4 above; none of
  them are mirrored into this repo.
- Once `yoke-config` lands, `yoke-volume` (C) consumes its types via the
  workspace dependency graph. The `Profile` type is the boundary.
- The Tauri IPC types in sub-project E will be the same `serde`-derived
  types this crate exposes. No DTOs.
