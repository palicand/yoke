# yokectl — bindings/preferences views, dynamic completions, proptest harness (addendum)

- **Date:** 2026-05-25
- **Status:** Drafted, awaiting review
- **Sub-project ID:** D (`yokectl`) — addendum to [2026-05-18-yokectl-design.md](./2026-05-18-yokectl-design.md)

## Context

Four gaps surfaced after the first round of yokectl use:

1. `show <target>` summarises bindings and preferences by count but cannot enumerate them. Operators repeatedly fall back to `pull` + reading the CSV.
2. Completion covers subcommands and flags only. Profile names, sub-profile names, catalog values, and community-index entry names are typed by hand even though the data is local.
3. The CLI is exercised by per-command integration tests and JSON snapshot tests, but not by generative testing. Sequences of edits across many commands are unverified.

A fourth request — a `device restart` command — is **out of scope** because true reboot requires the HID/serial transport that sub-project G owns (the QuadStick volume layer cannot issue a reset; in QMP-mac the only "reset" path is over serial, and that command is *reset outputs*, not a reboot). `device restart` joins `device push-live` / `device save-to-slot` / `device read-live` when G lands. No name is reserved in this addendum.

This addendum is yokectl-only — no new crates, no changes to `yoke-config`, `yoke-edit`, `yoke-index`, or `yoke-volume`. The proptest harness uses `FsBackend` through `--fake-volume` exactly as the existing integration tests do.

## Changes

### 1. `bindings` subcommand

Grammar:

```text
yokectl bindings <target> [--sub-profile <name>]
```

`<target>` follows the existing convention (volume profile name, file path, or `-` for stdin). Parses via `yoke_config::parse`, walks `profile.sub_profiles`, and prints bindings grouped by sub-profile.

Default human output:

```text
Cougar (mode=Joystick channel=Right)
  Sip Soft        -> Button A
  Sip Hard        -> Button B
  Puff Soft       -> Right Stick Up
  Lip Forward     -> Throttle Up

Falcon (mode=Joystick channel=Left)
  Sip Soft        -> Button X
  Lip Center      -> Throttle Center
```

`--sub-profile Cougar` filters to that block. Missing name → exit code 5 with `EditError::SubProfileNotFound` (reuses the existing `yoke-edit` error type since this is a read-only counterpart to `set-binding`).

Empty sub-profile (no bindings) prints `(no bindings)` in human mode, `"bindings": []` under JSON.

Default JSON shape:

```json
{
  "sub_profiles": [
    {
      "name": "Cougar",
      "mode": "Joystick",
      "channel": "Right",
      "sub_mode": null,
      "bindings": [
        {"input": "Sip Soft", "output": "Button A"},
        {"input": "Sip Hard", "output": "Button B"}
      ]
    }
  ]
}
```

`--sub-profile <name>` under `--json` returns the same shape with a single-element `sub_profiles` array — the schema is stable regardless of the filter.

Bindings are sorted by input phrase within each sub-profile; sub-profiles appear in declaration order from the source file.

### 2. `preferences` subcommand

Grammar:

```text
yokectl preferences <target> [--sub-profile <name>] [--raw]
```

Default (effective view) resolves each preference to the value the device sees: per-sub-profile override if present, else top-level value. Overridden entries are marked.

Default human output:

```text
Top-level:
  sip_soft_threshold        35
  puff_soft_threshold       38
  enable_DS3_emulation      false

Cougar:
  sip_soft_threshold        40              [override]
  puff_soft_threshold       38
  enable_DS3_emulation      false

Falcon:
  sip_soft_threshold        35
  puff_soft_threshold       42              [override]
  enable_DS3_emulation      false
```

`--sub-profile Cougar` prints the `Top-level` block plus only that sub-profile's resolved block. Missing sub-profile name → exit code 5 with `EditError::SubProfileNotFound`, same as `bindings`.

`--raw` switches to a layered view that mirrors the file structure — top-level prefs plus per-sub-profile overrides only, no resolution. `--raw` and `--sub-profile <name>` compose: the output is the `Top-level` block plus only that sub-profile's `(overrides)` block.

```text
Top-level:
  sip_soft_threshold        35
  puff_soft_threshold       38
  enable_DS3_emulation      false

Cougar (overrides):
  sip_soft_threshold        40

Falcon (overrides):
  puff_soft_threshold       42
```

Default JSON shape:

```json
{
  "top_level": {
    "sip_soft_threshold": 35,
    "puff_soft_threshold": 38,
    "enable_DS3_emulation": false
  },
  "sub_profiles": [
    {
      "name": "Cougar",
      "preferences": {
        "sip_soft_threshold": {"value": 40, "overridden": true},
        "puff_soft_threshold": {"value": 38, "overridden": false},
        "enable_DS3_emulation": {"value": false, "overridden": false}
      }
    }
  ]
}
```

`--raw` JSON shape:

```json
{
  "top_level": { "sip_soft_threshold": 35, "puff_soft_threshold": 38, "enable_DS3_emulation": false },
  "sub_profiles": [
    { "name": "Cougar", "overrides": { "sip_soft_threshold": 40 } },
    { "name": "Falcon", "overrides": { "puff_soft_threshold": 42 } }
  ]
}
```

Preference value types match `yoke_edit::PreferenceValue` (untagged number/bool/text) so the wire shape stays consistent with what `set-preference --json` emits today. Schemas pinned by `insta` snapshots.

### 3. View module

Both subcommands live in `crates/yokectl/src/commands/view.rs` (new). The module exports:

```rust
pub fn run_bindings(args: BindingsArgs, ctx: &Context) -> anyhow::Result<()>;
pub fn run_preferences(args: PreferencesArgs, ctx: &Context) -> anyhow::Result<()>;

// shared kernel
fn effective_preferences(profile: &Profile) -> EffectivePrefs;
fn group_bindings(profile: &Profile) -> GroupedBindings;
```

`effective_preferences` and `group_bindings` are pure functions over `Profile` — testable without I/O. The `_args` types are clap-derived structs added to `cli.rs`.

### 4. Dynamic completions via `CompleteEnv`

Enable the `unstable-dynamic` feature on `clap_complete` (via `cargo add clap_complete --features unstable-dynamic`). The `clap_complete` line in `Cargo.toml` is pinned to an exact version (`=4.6.x`) so a minor bump cannot break the build silently.

In `main.rs`, before `Cli::parse()`:

```rust
clap_complete::env::CompleteEnv::with_factory(Cli::command).complete();
```

When the shell invokes `yokectl` with `COMPLETE=$SHELL` set, this short-circuits before any real command runs. Installation, once per shell:

```text
# fish (any session)
COMPLETE=fish yokectl | source

# bash (~/.bashrc)
source <(COMPLETE=bash yokectl)

# zsh (~/.zshrc)
source <(COMPLETE=zsh yokectl)

# elvish, powershell — analogous
```

The existing static `yokectl completions <shell>` subcommand stays for users who want a generated script committed to a system path (`/etc/bash_completion.d/yokectl`, `$fpath[1]/_yokectl`, …). Static covers subcommand/flag enumeration; dynamic adds value sources. Both coexist; dynamic is the preferred install.

#### 4.1 Completers

Four `ArgValueCompleter` types under `crates/yokectl/src/completion/`:

| Completer | Attached to | Source |
|---|---|---|
| `ProfileNameCompleter` | `<target>`, `<name>`, `<from>`, `<to>` args referring to a volume profile | `VolumeProvider::list_profiles()` on the resolved backend |
| `SubProfileNameCompleter` | every `<sub-profile>` arg | parses the preceding `<target>` arg (if it exists on volume or disk), returns `sub_profiles[].name` |
| `CatalogValueCompleter` | `<input>`, `<output>`, `<key>`, `--mode`, `--channel`, `--sub-mode` args | `yoke_config::catalog::{inputs, outputs, preferences, modes, channels}` |
| `IndexEntryCompleter` | `install <source>`, `index show <name>` | on-disk `IndexClient` cache via a sync helper; no HTTP |

#### 4.2 Backend resolution in the completion path

`ProfileNameCompleter` calls `commands::resolve_backend_for_completion(argv)`:

1. Parses `--fake-volume` out of the already-typed argv tail.
2. If present → `FsBackend::new(path)` (sync, cheap).
3. If absent → platform backend with a hard 200 ms timeout. On macOS that's a single DiskArbitration snapshot. On Linux/Windows it returns empty immediately.

Completion-path failures are silent: empty candidate list, no stderr write, no log. The shell must not choke because the device is unplugged or the cache is missing.

#### 4.3 Index cache sync helper

`IndexClient` gains a sync, no-network helper:

```rust
impl IndexClient {
    pub fn read_cached_entries_sync(&self) -> Result<Vec<String>, IndexError>;
}
```

It reads `directories::ProjectDirs::from("com", "Yoke", "yokectl").cache_dir().join("index.csv")` directly. Returns `Ok(vec![])` if the cache file does not exist. No tokio.

#### 4.4 Feature flag exposure

`unstable-dynamic` is on by default in the `yokectl` crate's `Cargo.toml`. The exact-version pin on `clap_complete` is the hedge against upstream churn. The CI step that asserts each static completion is non-empty is extended with one additional step per shell that runs `COMPLETE=$SHELL yokectl` and asserts the output is non-empty — a smoke check that the dynamic wiring still produces installable shell code.

### 5. Property test harness

Lives under `crates/yokectl/tests/property/`, separate from existing `tests/integration/`. Targeting only properties: `cargo test -p yokectl --test property`.

#### 5.1 Action model

```rust
#[derive(Debug, Clone)]
enum Action {
    SetTitle           { profile: ProfileRef, title: String },
    SetPreference      { profile: ProfileRef, key: String, value: PreferenceValue },
    UnsetPreference    { profile: ProfileRef, key: String },
    SetOverride        { profile: ProfileRef, sub: SubRef, key: String, value: PreferenceValue },
    UnsetOverride      { profile: ProfileRef, sub: SubRef, key: String },
    SetBinding         { profile: ProfileRef, sub: SubRef, input: String, output: String },
    ClearBinding       { profile: ProfileRef, sub: SubRef, input: String },
    AddSubProfile      { profile: ProfileRef, name: String, mode: SubProfileMode, channel: Channel, sub_mode: Option<String> },
    DeleteSubProfile   { profile: ProfileRef, sub: SubRef },
    RenameSubProfile   { profile: ProfileRef, from: SubRef, to: String },
    CloneSubProfile    { profile: ProfileRef, from: SubRef, to: String },
    Push               { profile: ProfileRef, bytes: Vec<u8> },
    Pull               { profile: ProfileRef },
    Copy               { from: ProfileRef, to: String },
    Rename             { from: ProfileRef, to: String },
    Delete             { profile: ProfileRef, force: bool },
    Show               { profile: ProfileRef, raw: bool },
    Validate           { profile: ProfileRef },
    Bindings           { profile: ProfileRef, sub: Option<SubRef> },
    Preferences        { profile: ProfileRef, sub: Option<SubRef>, raw: bool },
    Apply              { profile: ProfileRef, ops: Vec<EditOp> },
    Install            { source: ProfileSource },
    CatalogInputs,
    CatalogOutputs,
    CatalogPreferences,
    CatalogModes,
    CatalogChannels,
    Device,
    List,
}
```

`ProfileRef` / `SubRef` are indices into the currently-existing set, so the strategy can produce both valid references and (rarely) invalid ones, proportionally biased toward valid so sequences make forward progress. Strings and values are drawn from a mix of catalog values (high frequency) and arbitrary unicode (low frequency).

#### 5.2 Dispatch

Each action converts to its real `Cli` parsed form and routes through the same `commands::*::run(args, ctx)` function the binary uses — no subprocess. `ctx` holds an `Arc<dyn VolumeProvider>` backed by `FsBackend` rooted at the tempdir. stdout and stderr are captured into `Vec<u8>` buffers per call.

`Install` with `ProfileSource::Url` uses loopback URLs; a `wiremock::MockServer` runs alongside the case. `IndexEntry` sources pre-seed the cache with synthetic entries pointing at the mock server. Real network is never reached.

#### 5.3 Invariants

One `proptest!` test per invariant:

1. **`prop_no_panics`** — Any `Action` sequence completes without panicking. Every `Result::Err` maps to an `i32` exit code in the §8 table of the parent spec. Shrinking reports the minimised sequence.

2. **`prop_round_trip_equality`** — For every action that successfully writes a profile, `yoke_config::parse(read_profile(name))` produces a `Profile` structurally equal to the in-memory one the CLI just wrote. Equality ignores whitespace-only differences in serialized fields. Catches serializer drift.

3. **`prop_apply_atomicity`** — For every `Apply` action with at least one invalid op interleaved with valid ones, the on-disk bytes after the call are byte-identical to the pre-call bytes and the exit code is 5. Catches partial-apply regressions.

4. **`prop_exit_and_json`** — Every action invoked twice (once without `--json`, once with) produces the same exit code in both invocations; the exit code is in the §8 table; under `--json`, stdout parses as a single JSON document (or NDJSON for `watch`-class actions) matching the per-command schema fixture under `insta`.

#### 5.4 Configuration and CI

`proptest!` config: 256 cases per invariant, `max_shrink_iters: 64`. Tempdir fresh per case. Single-threaded tokio runtime constructed on demand. `PROPTEST_CASES` env var overrides.

Runs in the existing `cargo test --workspace` job. No separate workflow. `proptest-regressions/` is committed under `tests/property/proptest-regressions/`; do not gitignore.

### 6. Tests for the new feature surface

#### 6.1 Unit (`view.rs`)

- `effective_preferences(profile)` table-driven: top-level value present and no override, top-level value present with sub-profile override, override present with no top-level value, key absent everywhere (not enumerated in output).
- `group_bindings(profile)` ordering: sub-profiles by declaration order, bindings sorted by input phrase.
- Empty profile (no sub-profiles, no bindings) renders without panic.

#### 6.2 Integration (`tests/integration/`)

- `bindings_basic` — fixture with two sub-profiles, default and `--sub-profile` filtered output.
- `bindings_empty` — sub-profile with no bindings.
- `bindings_missing_sub_profile` — exit code 5 + JSON error envelope.
- `bindings_json_schema` — `insta` JSON snapshots for default and filtered.
- `preferences_effective` — top-level + override fixture, `[override]` markers.
- `preferences_raw` — same fixture under `--raw`.
- `preferences_json_schema` — `insta` JSON snapshots for default, `--raw`, and `--sub-profile` filtered.

#### 6.3 Completion

- `completion_static_unchanged` — existing static-completion tests keep passing.
- `completion_dynamic_smoke` — `COMPLETE=bash` + `_CLAP_COMPLETE_INDEX=N` + partial argv, assert candidates contain expected entries. One test per completer.
- `completion_no_panic_on_missing_volume` — `--fake-volume` pointing at a non-existent path; `ProfileNameCompleter` returns empty silently, exit 0.
- `completion_no_panic_on_missing_cache` — `IndexEntryCompleter` against missing cache returns empty silently.
- `completion_honors_fake_volume` — candidates reflect `--fake-volume` directory contents, not platform default.

#### 6.4 Corpus

When `YOKE_CORPUS_DIR` is set, for each CSV in the corpus: execute `bindings <name>` and `preferences <name>` against `--fake-volume <tempdir>` and assert exit code 0 and parseable JSON. CI does not set the env var; this is a local-only run that catches real-world preference values inline fixtures miss.

## Acceptance criteria

1. `yokectl bindings <target> [--sub-profile <name>]` implemented with human and JSON output. Schemas pinned by `insta` snapshots.
2. `yokectl preferences <target> [--sub-profile <name>] [--raw]` implemented with human and JSON output. Schemas pinned by `insta` snapshots. Effective-resolution logic covered by `view.rs` unit tests.
3. `clap_complete::CompleteEnv` wired into `main` behind the `unstable-dynamic` feature flag. All four completers (`ProfileNameCompleter`, `SubProfileNameCompleter`, `CatalogValueCompleter`, `IndexEntryCompleter`) operate for bash, zsh, fish, elvish, powershell. `clap_complete` pinned to an exact version in `Cargo.toml`.
4. Completion path is silent on missing volume, missing cache, or unresolvable target: empty candidates, no panic, no stderr.
5. Static `yokectl completions <shell>` subcommand still produces non-empty output for each shell (regression).
6. Four proptest invariants (`prop_no_panics`, `prop_round_trip_equality`, `prop_apply_atomicity`, `prop_exit_and_json`) run as part of `cargo test --workspace` at 256 cases each. The harness covers every command in the `Action` enum, including the new `Bindings` and `Preferences` commands.
7. `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check` clean on macOS + Linux.
8. `cargo build -p yoke-config --target wasm32-unknown-unknown` and `cargo build -p yoke-edit --target wasm32-unknown-unknown` still pass.
9. The catalog-drift guard test in `yoke-edit` still passes.
10. Maintainer-validated smoke pass against a real QuadStick on macOS, recorded in the PR description:
    - `bindings <volume-profile>` and `preferences <volume-profile>` produce sensible output for the live device.
    - Fresh shell, `COMPLETE=fish yokectl | source`, then `yokectl bindings <TAB>` autocompletes volume profile names, `yokectl set-binding <profile> <TAB>` autocompletes sub-profile names, `yokectl install <TAB>` autocompletes from the cached community index.

## Out of scope

- **Device restart.** Deferred to sub-project G alongside `device push-live` / `device save-to-slot` / `device read-live`. No name reserved here.
- **TUI / interactive picker** for the bindings and preferences views — these are read-only commands; interactive editing belongs to the GUI (sub-project F).
- **Completion of arbitrary file paths** for the `<target>` argument — clap's built-in `ValueHint::FilePath` already covers this and is not regressed.

## Forward references

- The `Action` enum and in-process dispatch helper in `tests/property/` are the canonical test substrate for future yokectl features — new commands ship with an `Action` variant and ride the existing invariants for free.
- `IndexClient::read_cached_entries_sync` is reused by the GUI (sub-project F) for its "install from community index" picker — same on-disk cache, no separate read path.
- When sub-project G lands, `device restart` is wired alongside the other live-device commands and the proptest harness gains a `DeviceRestart` action variant.

## Known risks

- `clap_complete`'s `unstable-dynamic` is pre-1.0 by design; a minor crate bump may break the build. The exact-version pin in `Cargo.toml` contains the risk. Each upgrade is a separate maintenance task that re-runs the completion smoke tests before merging.
- `ProfileNameCompleter` calls into the platform backend with a 200 ms timeout when `--fake-volume` is absent. On macOS this is the DiskArbitration snapshot path; if a future macOS slows it materially, raise the timeout or degrade earlier.
- Proptest determinism depends on `tests/property/proptest-regressions/` being committed. Do not gitignore.
