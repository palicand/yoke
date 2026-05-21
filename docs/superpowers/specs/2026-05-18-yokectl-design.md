# yokectl — design

- **Date:** 2026-05-18
- **Status:** Approved, ready for implementation plan
- **Sub-project ID:** D (`yokectl`)

## Context

Yoke needs a command-line surface that covers every interaction with a QuadStick a human or agent could want, short of firmware flashing (deferred to stage J behind explicit safeguards). The volume layer (sub-project C, `yoke-volume`) and the configuration model (sub-project B, `yoke-config`) are done; this sub-project is the first end-user-visible deliverable and the substrate that future-Claude — and any other agent — will use to exercise the full edit/save/load loop without a native window.

The CLI consumes the existing crates and adds three things that don't yet exist:

1. A library of **typed edit operations** over `yoke_config::Profile` that the GUI will later share (binding edits, preference overrides, sub-profile lifecycle).
2. A **community-index + Google-Sheets fetch** layer so users can install configs by name or URL instead of hand-downloading CSVs.
3. The **binary itself**: clap dispatch, output formatting, error mapping, exit codes, shell-completion generation.

The sub-project also extends `yoke-volume`'s `FsBackend` with a small `test-utils` feature-gated API so the CLI's event-stream commands (`watch`, `device`) can be exercised in CI without a real device.

## Goals

1. Three new crates: `yoke-edit` (host + wasm32, pure edit ops), `yoke-index` (host-only, community-index + HTTP), `yokectl` (binary, host-only).
2. Every operation supported by the existing crates is reachable from the CLI — list, show, validate, push, pull, copy, rename, delete profiles; set/clear bindings; set/unset preferences and per-sub-profile overrides; add/delete/rename/clone sub-profiles; observe device and volume state.
3. `install <source>` resolves a profile from the community index, a Google Sheets URL, or a local CSV, validates it, and writes it to the volume — three input shapes, one command.
4. `--fake-volume <path>` makes the entire CLI runnable against an `FsBackend` with no device — every test and every agent flow uses this path.
5. JSON output (`--json`) is stable per-command; exit codes are pinned by integration test; both are documented per command.
6. Shell-completion generation for bash, zsh, fish, powershell, elvish — works on first install via a single `completions <shell>` subcommand.
7. The command grammar reserves names for future device-channel operations (HID over USB, sub-project G) so the surface doesn't shift when those land.

## Non-goals

- **No HID, serial, or device-channel transport.** Sub-project G (`yoke-device`) lands those. `yokectl` reserves names like `device push-live` but they return `not yet implemented` until G ships.
- **No firmware flashing.** Sub-project J, far future, behind its own crate and explicit safeguards.
- **No OAuth or Drive API integration.** Google-Sheets fetch is restricted to publicly accessible sheets (published-to-web or anyone-with-link). Private configs are downloaded by the user manually, then `push`/`install`-ed.
- **No interactive TUI.** clap dispatch only. The interactive editor is the GUI's job (sub-project F).
- **No package-manager installation (Homebrew tap, MSI installer, etc.).** Sub-project K territory. Release artifacts may be produced manually; distribution channels are not in scope here.
- **No Linux backend coverage.** `yoke-volume-linux` lands when there's a maintainer for it. On Linux, `yokectl` builds and `--fake-volume` works; without the flag it errors out with a backend-missing diagnostic.
- **No GUI integration.** `yoke-edit` and `yoke-index` are shaped to be re-usable by `yoke-tauri`, but the GUI consuming them is sub-project E/F work.

## Design

### 1. Workspace layout

```text
yoke/crates/
├── yoke-config/            existing — model, parser, writer, vocabulary catalog
├── yoke-volume/            existing — VolumeProvider trait + FsBackend
├── yoke-volume-macos/      existing — DiskArbitration + IOKit backend
├── yoke-edit/              new   — host + wasm32 — pure edit operations
├── yoke-index/             new   — host-only — community index + Google-Sheets fetch
└── yokectl/                new   — bin, host-only, clap-based
```

Workspace `members` grows from `["crates/yoke-config", "crates/yoke-volume", "crates/yoke-volume-macos"]` to add `crates/yoke-edit`, `crates/yoke-index`, `crates/yokectl`.

Dependency graph (new edges only):

```text
yoke-edit    -> yoke-config
yoke-index   -> (no internal deps; pure fetch + parse)
yokectl      -> yoke-config, yoke-volume, yoke-edit, yoke-index
yokectl      -> yoke-volume-macos   [cfg(target_os = "macos")]
yoke-volume  -> (no change, but gains a `test-utils` feature; see § 13)
```

`yoke-edit` deps (via `cargo add`):

- `yoke-config` — workspace path.
- `serde` with `["derive"]` — the `EditOp` enum and JSON I/O for `apply --edits`.
- `thiserror` — workspace rule.
- `strsim` — Levenshtein for "did you mean…" suggestions.

`yoke-index` deps:

- `reqwest` with `["rustls-tls", "gzip"]` and `default-features = false` — async HTTP, no system OpenSSL. tokio is already a `yokectl` dep so the async runtime is reused.
- `url` — URL parsing for the Google-Sheets transformation pipeline.
- `csv` — workspace already uses the same crate for QuadStick CSVs; reused here for the index sheet.
- `serde` with `["derive"]` — `IndexEntry` shape.
- `thiserror` — workspace rule.
- `tracing` — workspace rule.
- `directories` — XDG-correct cache path on macOS/Linux/Windows.

`yokectl` deps:

- `yoke-config`, `yoke-volume`, `yoke-edit`, `yoke-index` — workspace paths.
- `yoke-volume-macos` — under `[target.'cfg(target_os = "macos")'.dependencies]`.
- `clap` with `["derive"]` — argument parsing and `--help`.
- `clap_complete` — `completions <shell>` generation.
- `clap_mangen` — man-page generation (used in tests; binary artifact is not yet shipped).
- `anyhow` — binary-side error chains.
- `tokio` with `["rt", "macros", "sync"]` — current-thread runtime for the async commands only.
- `tracing` + `tracing-subscriber` with `["fmt", "env-filter"]` — stderr-only logging.
- `serde` and `serde_json` — JSON output and the `apply --edits` reader.
- `is-terminal` — color / no-color decision.
- `console` — minimal ANSI styling (no `crossterm`; we don't need TTY input).

`yokectl` dev-deps:

- `assert_cmd` — binary-invocation harness.
- `predicates` — stream assertions.
- `insta` — JSON snapshot tests.
- `wiremock` — HTTP mocking for `install` / `index *` tests.
- `tempfile` — tempdirs for `--fake-volume`.

### 2. Backend selection and the `--fake-volume` flag

`--fake-volume <path>` is the only knob that picks the backend. Its presence picks `FsBackend`; its absence picks the platform default.

| Platform | `--fake-volume` absent | `--fake-volume <path>` present |
|---|---|---|
| macOS | `MacOsVolumeProvider::new()` | `FsBackend::new(path)` |
| Linux | error: `backend not yet available; use --fake-volume <path>` | `FsBackend::new(path)` |
| Windows | same as Linux until sub-project H | `FsBackend::new(path)` |

`backend::open(args) -> anyhow::Result<Arc<dyn VolumeProvider>>` lives in `yokectl/src/backend.rs` and is the only place that branches on `cfg(target_os = ...)`. Every command consumes `Arc<dyn VolumeProvider>` and is platform-agnostic.

There is no separate `--autodetect` or `--volume <real-path>` flag. If `MacOsVolumeProvider` ever fails to discover a present QuadStick on macOS, `--fake-volume /Volumes/QUADSTICK` is the documented escape hatch — the FAT operations against the mount point are identical, only the event-stream surface is lost.

### 3. Command grammar

Top-level flags (apply to every subcommand unless noted):

| Flag | Effect |
|---|---|
| `--fake-volume <path>` | Use `FsBackend` rooted at `<path>` instead of the platform backend. |
| `--json` | Emit machine-readable output on stdout. Single document for one-shot commands; NDJSON (one document per line) for `watch`. |
| `-v`, `-vv`, `-vvv` | Increase `tracing-subscriber` verbosity (stderr). `-vv` reveals `info`, `-vvv` reveals `debug`/`trace`. |
| `--no-color` | Disable ANSI styling. `NO_COLOR=1` and a non-TTY stdout also disable. |
| `-h`, `--help` | Standard clap help. |
| `-V`, `--version` | Print version. |

Subcommands grouped by purpose. **`<target>`** below is *any* of: a profile name on the volume (default), a local file path (auto-classified when the argument exists on disk), or `-`/`--stdin` to read bytes from stdin.

#### 3.1 Device state

| Command | Effect |
|---|---|
| `device` | Print current `MountState`: VID/PID, location-ID, mode hint, mount point if present. One-shot. |
| `debug` | Rich diagnostic snapshot: device fields, current `MountState`, BSD names, `/Volumes/` enumeration, every USB device the macOS backend saw on its last poll, parser-warning counts on every profile present. JSON-friendly; human-pretty by default. |
| `watch` | Stream `MountState` transitions and `MountEvent`s as they arrive. NDJSON under `--json`. `--include-poll` adds raw poll-tick events for forensics (requires `--json`; refused in human mode because the volume is too high to render usefully). |

#### 3.2 Profile I/O

| Command | Effect |
|---|---|
| `list` | List profiles on the volume (or the directory passed to `--fake-volume`). Columns: name, kind (Default/Prefs/Game), size, modified. |
| `show <target>` | Pretty-print parsed profile structure: title, sub-profiles with mode/sub-mode/channel, binding count, preference count. `--raw` skips parsing and emits the bytes verbatim. |
| `validate <target>` | Parse `<target>` and emit warnings/errors. Exit code 4 on parse error, 0 with warnings, 0 clean. |
| `pull <name> [dest]` | Copy a volume-backed profile to a local path. `dest` defaults to `./<name>.csv`. `--raw` skips re-serialization (byte-identical copy via `read_profile`). |
| `push <src> [name]` | Copy a local file to the volume. `name` defaults to the source's filename stem. No parse / validate by default; pass `--validate` to opt in. |
| `copy <from> <to>` | In-place copy on the volume. |
| `rename <from> <to>` | In-place rename on the volume. |
| `delete <name>` | Delete a profile from the volume. `--force` skips the "are you sure" prompt; `--json` implies `--force`. |

#### 3.3 Edit (preferences and bindings)

Each command constructs a one-element `Vec<EditOp>`, applies via `yoke_edit::apply`, and writes the result back to the target.

| Command | Effect |
|---|---|
| `set-title <target> <title>` | Replace `Profile.top_line.title`. |
| `set-preference <target> <key> <value>` | Set a top-level preference. Value type inferred from `catalog::preferences`. |
| `unset-preference <target> <key>` | Remove a top-level preference. |
| `set-override <target> <sub-profile> <key> <value>` | Set a per-sub-profile preference override. |
| `unset-override <target> <sub-profile> <key>` | Remove a per-sub-profile override. |
| `set-binding <target> <sub-profile> <input> <output>` | Bind an input phrase to an output. Both strings validated against the catalog. |
| `clear-binding <target> <sub-profile> <input>` | Remove a binding. |

#### 3.4 Sub-profiles

| Command | Effect |
|---|---|
| `subprofile add <target> <sub-profile> --mode <m> --channel <c> [--sub-mode <s>]` | Append a new sub-profile. Mode and channel validated against catalog. |
| `subprofile delete <target> <sub-profile>` | Remove a sub-profile. Errors if it is the last sub-profile in the file. |
| `subprofile rename <target> <from> <to>` | Rename a sub-profile (header only). |
| `subprofile clone <target> <from> <to>` | Duplicate a sub-profile under a new name. |

#### 3.5 Batch edits

| Command | Effect |
|---|---|
| `apply <target> --edits <file.json>` | Read a JSON document `{"edits": [EditOp, …]}`, apply atomically via `yoke_edit::apply`, write back. `--dry-run` validates without writing. `<file.json>` may be `-` for stdin. |

#### 3.6 Catalog inspection

| Command | Effect |
|---|---|
| `catalog inputs` | Enumerate valid input phrases (categorical, e.g. sip/puff/lip-position variants). |
| `catalog outputs` | Enumerate valid output names. |
| `catalog preferences` | List preference keys with declared value types. |
| `catalog modes` | Enumerate `SubProfileMode` values. |
| `catalog channels` | Enumerate `Channel` values. |

#### 3.7 Install from external sources

| Command | Effect |
|---|---|
| `install <source> [--as <name>]` | Auto-classify `<source>`: local path → read; URL → fetch (Google-Sheets rewrite if applicable); bare name → resolve via `yoke-index`. Parse, validate, write to the volume. `--as` overrides the destination filename. `--dry-run` parses and prints destination but does not write. `--no-validate` is an escape hatch; prints a warning to stderr. |

#### 3.8 Community index

| Command | Effect |
|---|---|
| `index list` | Cached community index as a table. `--refresh` forces a fetch. |
| `index search <query>` | Fuzzy filter over `name`. |
| `index show <name>` | Print the resolved row, including the underlying CSV URL. |
| `index update` | Force-refresh the cache. |

#### 3.9 Device-channel (reserved names)

| Command | Effect |
|---|---|
| `device push-live <target>` | Reserved for sub-project G. Returns exit code 1 with `{"code": "not-implemented", "stage": "G"}`. |
| `device save-to-slot <slot>` | Reserved. |
| `device read-live` | Reserved. |
| `protocol monitor [--hex]` | Reserved for HID 0xFF00 / serial dumping. |

Reserving these names now means agents can build flows against the planned grammar; only the implementation behind them changes when G ships.

#### 3.10 Completions

| Command | Effect |
|---|---|
| `completions <shell>` | Print a completion script to stdout. `<shell>` ∈ `{bash, zsh, fish, powershell, elvish}`. |

Dynamic completion of profile names and catalog values is not in this sub-project; static completion covers subcommand/flag enumeration which is the 80% case. Dynamic adds shell-side callbacks that invoke `yokectl` for value lists, which complicates the install story; deferred.

### 4. `yoke-edit` — the EditOp model

```rust
// yoke_edit::op

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum EditOp {
    SetTitle              { title: String },
    SetPreference         { key: String, value: PreferenceValue },
    UnsetPreference       { key: String },
    SetBinding            { sub_profile: String, input: String, output: String },
    ClearBinding          { sub_profile: String, input: String },
    SetOverride           { sub_profile: String, key: String, value: PreferenceValue },
    UnsetOverride         { sub_profile: String, key: String },
    AddSubProfile         { name: String, mode: SubProfileMode, sub_mode: String, channel: Channel },
    DeleteSubProfile      { name: String },
    RenameSubProfile      { from: String, to: String },
    CloneSubProfile       { from: String, to: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum PreferenceValue {
    Number(i64),
    Bool(bool),
    Text(String),
}
```

`PreferenceValue` is a small sum because preferences in `yoke-config`'s catalog are either numeric (thresholds, deadbands), boolean (enable/disable flags), or named choices. Untagged serialization keeps the JSON shape natural for agents: `"value": 35` not `"value": {"number": 35}`.

#### 4.1 Apply semantics

```rust
pub fn apply(profile: Profile, ops: &[EditOp]) -> Result<Profile, ApplyError>;

pub struct ApplyError {
    pub index: usize,           // which op in the batch
    pub error: EditError,
}
```

`apply` is **all-or-nothing**. It clones the input, walks the op list, and for each op:

1. Validates against the current (clone-) profile state.
2. Applies the mutation.
3. On failure, returns `ApplyError { index, error }` and drops the clone — the caller's `profile` is untouched.

Validation is progressive: op N validates against profile-after-ops-1..N-1, which allows sequences like `[AddSubProfile { name: "Cougar" }, SetBinding { sub_profile: "Cougar", ... }]` to succeed.

Cost is one `Profile::clone()` per `apply` call. Profiles are a few hundred small strings — negligible compared to the disk I/O on either side.

#### 4.2 Catalog-grounded validation

Every op consults `yoke_config::catalog` during validate:

- `SetBinding { input, output, .. }`: `input` must be in `catalog::inputs`; `output` must be in `catalog::outputs`. Unknown values surface as `EditError::UnknownInput { input, suggestions }` or `EditError::UnknownOutput { output, suggestions }`, where `suggestions: Vec<String>` is the set of catalog entries within Levenshtein distance 2 (via `strsim`), capped at 5.
- `SetPreference { key, value }`: `key` must be in `catalog::preferences`. The catalog declares each preference's expected type (`Number`, `Bool`, `Text(allowed_values)`); mismatched values surface as `EditError::InvalidPreferenceValue { key, value: String, expected_type: String }`.
- `AddSubProfile { mode, channel, .. }`: `mode` must be in `catalog::sub_profile_modes`; `channel` in `catalog::channels`. (These are typed enums so the parser already enforces this — the validation step is for completeness when the values arrive through serde JSON.)
- `RenameSubProfile`, `CloneSubProfile`, `DeleteSubProfile`, `SetBinding`, `ClearBinding`, `SetOverride`, `UnsetOverride`: the named `sub_profile` must exist in `profile.sub_profiles`. Missing → `EditError::SubProfileNotFound { name }`.
- `AddSubProfile`, `CloneSubProfile`, `RenameSubProfile (target)`: the new name must not already exist. Conflict → `EditError::SubProfileExists { name }`.

#### 4.3 Error type

```rust
#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("unknown input: {input:?}; did you mean: {suggestions:?}")]
    UnknownInput { input: String, suggestions: Vec<String> },
    #[error("unknown output: {output:?}; did you mean: {suggestions:?}")]
    UnknownOutput { output: String, suggestions: Vec<String> },
    #[error("unknown preference key: {key:?}; did you mean: {suggestions:?}")]
    UnknownPreference { key: String, suggestions: Vec<String> },
    #[error("preference {key}: value {value:?} is not a valid {expected_type}")]
    InvalidPreferenceValue { key: String, value: String, expected_type: String },
    #[error("sub-profile not found: {name:?}")]
    SubProfileNotFound { name: String },
    #[error("sub-profile already exists: {name:?}")]
    SubProfileExists { name: String },
    #[error("cannot delete the last remaining sub-profile")]
    LastSubProfileDeletion,
}
```

#### 4.4 Targets

`yoke-edit` builds on both host and `wasm32-unknown-unknown`. The crate forbids unsafe and has no I/O, no platform code, no time, no random. CI builds it for WASM in the same job that already builds `yoke-config` for WASM.

### 5. `yoke-index` — community index + Google Sheets fetch

#### 5.1 Public API

```rust
pub struct IndexClient {
    http: reqwest::Client,
    cache_path: PathBuf,
    cache_ttl: Duration,
}

impl IndexClient {
    pub fn new() -> Result<Self, IndexError>;
    pub fn with_cache(cache_path: PathBuf, ttl: Duration) -> Result<Self, IndexError>;

    pub async fn list(&self, refresh: bool) -> Result<IndexListing, IndexError>;
    pub async fn resolve(&self, name: &str) -> Result<IndexEntry, IndexError>;
    pub async fn fetch_profile(&self, src: ProfileSource) -> Result<Vec<u8>, IndexError>;
}

pub enum ProfileSource {
    LocalPath(PathBuf),
    Url(Url),
    IndexEntry(String),
}

pub struct IndexEntry {
    pub name: String,
    pub csv_url: Url,
    pub fields: std::collections::BTreeMap<String, String>,
}

pub struct IndexListing {
    pub entries: Vec<IndexEntry>,
    pub skipped: usize,
}
```

`fetch_profile` is the unified entry point: local path → `tokio::fs::read`; `Url` → `http.get(transform(url))` → bytes; `IndexEntry(name)` → `resolve` then fetch the entry's URL. Tracing spans wrap every fetch; `tracing::warn!` fires when the cache is stale by more than `ttl`.

#### 5.2 Canonical index URL

```rust
pub const COMMUNITY_INDEX_URL: &str =
    "https://docs.google.com/spreadsheets/d/e/\
     2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/\
     pub?gid=1483029791&single=true&output=csv";
```

The constant is the CSV-export form of the maintainer's link. Updating the URL requires a `yoke-index` release; this is on purpose so users don't get silently redirected.

#### 5.3 URL transformation

| Input shape | Transformed to |
|---|---|
| `…/d/e/{KEY}/pubhtml?gid={GID}&single=true` (published HTML) | `…/d/e/{KEY}/pub?gid={GID}&single=true&output=csv` |
| `…/d/e/{KEY}/pub?...&output=csv` (already CSV) | unchanged |
| `…/d/{KEY}/edit#gid={GID}` (anyone-with-link) | `…/d/{KEY}/export?format=csv&gid={GID}` |
| Other `docs.google.com/spreadsheets/...` | matched on path segments; missing key/gid is `IndexError::InvalidUrl` |
| Non-`docs.google.com` URL | unchanged; GET expects `text/csv` or `text/plain` |

The transformer is a pure function with unit tests over each row of this table. A failing GET response (non-2xx, wrong content-type) returns `IndexError::FetchFailed { url, status }`.

#### 5.4 Index CSV parsing

The index is a published Google Sheet. yoke-index parses it as CSV with header row matching:

- Required columns: a column whose header's last whitespace-separated token is `name` (case-insensitive) — this matches both the bare `Name` header and qualified variants like `Configuration Spreadsheet Name` used by the live upstream sheet — and a URL column picked by priority: a header containing the word `url` wins, then `link`, then any header containing the substring `csv`. The priority ordering disambiguates sheets that carry both a fetchable URL column and a separate filename column (e.g. `CSV Filename` alongside `Spreadsheet URL`).
- All other columns become `IndexEntry.fields` (preserved verbatim).
- Rows with an empty name or unparseable URL are skipped with a `tracing::warn!` and counted; the count is surfaced by `index list --json` so downstream consumers can detect index drift.
- Header matching is name-based (not positional) so the upstream sheet can add or reorder columns without breaking the parser.

#### 5.5 Cache

- Path: `directories::ProjectDirs::from("com", "Yoke", "yokectl").cache_dir().join("index.csv")`.
- TTL: 24 h. `--refresh` on any `index` or `install` invocation forces a fetch and rewrites the cache.
- Cache I/O is `tokio::fs` async; failures (missing dir, permissions) downgrade to in-memory-only mode with a `tracing::warn!` — `index list` still works, just doesn't persist across runs.

#### 5.6 Error type

```rust
#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("index URL not parseable: {0}")]
    InvalidUrl(String),
    #[error("fetch failed: {url}: HTTP {status}")]
    FetchFailed { url: Url, status: u16 },
    #[error("index format unexpected: {0}")]
    IndexFormat(String),
    #[error("no index entry matching name: {0}")]
    NotFound(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
```

### 6. `install` — composition of the above

`install <source>` is the user-facing top of the stack. Auto-classification:

1. If `<source>` exists on disk as a file → `ProfileSource::LocalPath`.
2. Else if `<source>` parses as a URL with an `http(s)` scheme → `ProfileSource::Url`.
3. Else → `ProfileSource::IndexEntry(name)`.

Algorithm:

```text
install(source, as_name, dry_run, no_validate):
    bytes      <- yoke_index::IndexClient::fetch_profile(source)
    profile    <- yoke_config::parse(bytes)
    if not no_validate:
        if profile has parse errors: fail with code 4
        emit profile warnings to stderr
    dest_name  <- as_name or default-derived (index entry name, URL basename, or local stem)
    if dry_run: print {"action": "would-install", "dest": dest_name}; exit 0
    provider.write_profile(dest_name, bytes)
    print {"action": "installed", "dest": dest_name}; exit 0
```

`--no-validate` writes the bytes without parsing. A `tracing::warn!` is emitted; the JSON output for the action carries `"validated": false` so downstream tooling can detect it.

### 7. Output formats

#### 7.1 Streams

- **stdout**: command data. Either human-formatted (default) or JSON (`--json`).
- **stderr**: tracing output (logs, warnings, progress), clap usage errors, and `--help` text.

This split is what makes `yokectl list --json | jq '.[] | .name'` and `yokectl pull foo.csv | sha256sum` work without unnecessary escaping.

#### 7.2 Human format

- ANSI color when `stdout.is_terminal()` && `NO_COLOR` unset && `--no-color` absent.
- Tables use simple aligned columns (no box-drawing characters).
- Single-value outputs (e.g. `pull` to stdout, `show <name> --raw`) print bytes only; no trailing newline added beyond what the data carries.

#### 7.3 JSON format

- One JSON document per one-shot command, written to stdout, followed by a single newline.
- NDJSON (one document per line) for `watch`. Each line is a self-contained JSON object with a `kind` discriminator (`mount-state` for state snapshots, `mount-event` for transitions).
- Schemas are documented in the per-command `--help`. JSON snapshot tests under `insta` pin every schema; schema changes require explicit snapshot review.
- Floating-point numbers, when present, use JSON's natural representation. Times are ISO-8601 UTC strings.

#### 7.4 `watch` output sketch

Human, default:

```text
[2026-05-18T14:23:01Z] DeviceAppeared      vid=16D0:092B
[2026-05-18T14:23:01Z] VolumeMounted       mount=/Volumes/QUADSTICK label=QUADSTICK
[2026-05-18T14:25:14Z] DeviceModeChanged   vid=0F0D:0066 hint=Ps4OrHori
[2026-05-18T14:25:14Z] VolumeUnmounted
[2026-05-18T14:25:14Z] state: DeviceVisibleNoVolume vid=0F0D:0066 hint=Ps4OrHori
```

JSON (NDJSON), one line per row above:

```text
{"timestamp":"2026-05-18T14:23:01Z","kind":"mount-event","event":{"kind":"DeviceAppeared","vid_pid":{"vendor":5840,"product":2347}}}
{"timestamp":"2026-05-18T14:23:01Z","kind":"mount-event","event":{"kind":"VolumeMounted","mount_point":"/Volumes/QUADSTICK","label":"QUADSTICK","vid_pid":{"vendor":5840,"product":2347}}}
{"timestamp":"2026-05-18T14:25:14Z","kind":"mount-event","event":{"kind":"DeviceModeChanged","vid_pid":{"vendor":3853,"product":102},"mode_hint":"Ps4OrHori"}}
{"timestamp":"2026-05-18T14:25:14Z","kind":"mount-event","event":{"kind":"VolumeUnmounted"}}
{"timestamp":"2026-05-18T14:25:14Z","kind":"mount-state","state":{"kind":"DeviceVisibleNoVolume","vid_pid":{"vendor":3853,"product":102},"mode_hint":"Ps4OrHori"}}
```

State snapshots are emitted in addition to events whenever a transition changes `MountState`; this lets a JSON consumer track current state without re-deriving it from events.

### 8. Exit-code mapping

| Code | Class | Root cause |
|---|---|---|
| 0 | Success | command completed |
| 1 | Generic | unclassified anyhow chain, `not-implemented` reserved commands |
| 2 | Argument | clap parse failure, `VolumeError::InvalidProfileName` |
| 3 | Device | `VolumeError::NotPresent`, `VolumeError::VolumeHidden` |
| 4 | Parse | `yoke_config::ParseError`, `IndexError::IndexFormat` |
| 5 | Edit | `yoke_edit::EditError::*` |
| 6 | I/O | `VolumeError::Io`, `IndexError::Io`, top-level `std::io::Error` |
| 7 | Network | `IndexError::Network`, `IndexError::FetchFailed`, `IndexError::InvalidUrl`, `IndexError::NotFound` |
| 64–127 | Reserved | `yoke-device` (sub-project G) failure classes |

`yokectl/src/error.rs` performs the mapping: walks `anyhow::Error::chain()`, downcasts to each known structured root type in priority order, picks the first match. The mapping table is the authoritative ordering; the integration test in § 14.5 pins it.

#### 8.1 JSON error envelope

```json
{
  "error": {
    "code": "edit-unknown-input",
    "message": "unknown input: \"Sip Sof\"; did you mean: [\"Sip Soft\", \"Sip Hard\"]",
    "details": {
      "input": "Sip Sof",
      "suggestions": ["Sip Soft", "Sip Hard"]
    }
  }
}
```

`code` is kebab-case and stable. `message` is the human-formatted error chain. `details` carries the structured fields from the underlying error; per-code shape is fixed by the snapshot suite.

Errors under `--json` go to **stdout**, not stderr, because JSON consumers read a single stream. Logs and tracing output still go to stderr.

### 9. Shell completion

```text
yokectl completions bash       > /etc/bash_completion.d/yokectl
yokectl completions zsh        > $fpath[1]/_yokectl
yokectl completions fish       > ~/.config/fish/completions/yokectl.fish
yokectl completions powershell > $PROFILE
yokectl completions elvish     > ~/.config/elvish/lib/yokectl.elv
```

Generation is delegated entirely to `clap_complete::generate`. The subcommand has no platform `cfg`s — every shell generator is available on every platform. A test asserts the bash output starts with `_yokectl()` and the fish output contains `complete -c yokectl`.

Dynamic completion (profile names, catalog values) is **out of scope**; a future sub-project may revisit by adding a `--complete <shell> <args>...` hidden command that the static scripts call back into.

### 10. Debug, raw, and watch verbosity

| Affordance | Behavior |
|---|---|
| `debug` | One-shot snapshot. Human: a multi-section pretty-printed report. JSON: a single document with sections as keys (`device`, `mount`, `volumes`, `usb_devices`, `profiles`). |
| `show --raw` / `pull --raw` | Bypass parser; emit volume bytes verbatim. `pull --raw <name> -` writes to stdout for piping. |
| `-v` / `-vv` / `-vvv` | Increase `tracing-subscriber` level (`warn` → `info` → `debug` → `trace`). |
| `watch --include-poll` | Emit poll-tick events from `MacOsVolumeProvider`'s timer in addition to public events. NDJSON-only; refused in human mode (the volume is too high to render usefully). |

### 11. Internal architecture

#### 11.1 Binary layout

```text
crates/yokectl/src/
├── main.rs               entrypoint: tracing setup, dispatch, exit-code mapping
├── cli.rs                clap derive: Cli, Commands, every subcommand struct
├── output.rs             Human vs JSON renderers, color/no-color, NDJSON for streams
├── source.rs             ProfileSource classification (path / URL / name)
├── target.rs             EditTarget classification (volume-name / file / stdin)
├── backend.rs            VolumeProvider construction (FsBackend vs Mac vs future)
├── runtime.rs            Lazy current-thread tokio runtime for async commands only
├── error.rs              Top-level anyhow::Error → ExitCode + JSON envelope
└── commands/
    ├── mod.rs
    ├── device.rs         device, watch, debug
    ├── profile.rs        list, show, validate, pull, push, copy, rename, delete
    ├── edit.rs           set-title, set-preference, unset-preference, set-binding,
    │                     clear-binding, set-override, unset-override
    ├── subprofile.rs     subprofile add/delete/rename/clone
    ├── apply.rs          apply --edits <file.json>
    ├── install.rs        install <source>
    ├── index.rs          index list/search/show/update
    ├── catalog.rs        catalog inputs/outputs/preferences/modes/channels
    └── completions.rs    completions <shell>
```

#### 11.2 Async strategy

`main` is synchronous and dispatches to either a sync command function or `runtime::block_on(async { … })`. The current-thread tokio runtime is constructed only when needed (`watch`, every `install` / `index *` invocation). Sync commands (`list`, `show`, `push`, `pull`, every `set-*`, `apply`, `catalog *`, `completions`, `device`, `debug`) never instantiate tokio — startup is fast for the common cases the agent flows hit.

The `VolumeProvider` trait's I/O methods are sync; the watch/broadcast subscriptions return tokio receivers but they can be `.recv().await`-ed only inside a runtime, which is exactly the `watch` command's path.

#### 11.3 Command dispatch contract

Each subcommand module exports a single function:

```rust
pub fn run(args: Args, ctx: &Context) -> anyhow::Result<()>;
```

or, for async-only commands:

```rust
pub async fn run(args: Args, ctx: &Context) -> anyhow::Result<()>;
```

`Context` carries the `Arc<dyn VolumeProvider>`, the output renderer (human or JSON), and the IndexClient (lazy-constructed). This is the only allocation surface — no global state, no `lazy_static`.

### 12. Error-handling consolidation

The error story is small and well-defined:

- **Library crates** (`yoke-edit`, `yoke-index`) export `thiserror` enums with structured variants. Library consumers (CLI now, GUI later) pattern-match on these to react.
- **Binary crate** (`yokectl`) uses `anyhow::Result` throughout. Every operation that crosses a library boundary uses `.context("while reading {target}")` so the human error message is a stack of breadcrumbs.
- **Top-level mapping** in `yokectl/src/error.rs` walks `anyhow::Error::chain()` and downcasts to known structured types in the priority order from the § 8 table. First match wins. Unmatched chains fall through to exit code 1 with `error.code = "internal"`.

Why this split: structured errors at library boundaries give the GUI (which is a different consumer) the same ability to react. `anyhow` at the binary boundary collapses the heterogeneous error universe into one type that the dispatcher can map mechanically.

### 13. `yoke-volume` test-utils hook (cross-crate addition)

The volume crate is sub-project C territory and its spec is frozen, but this sub-project requires one small additive change to `FsBackend` so `watch` and `device` are testable in CI. It is **additive**: it does not alter any existing API and lives behind a Cargo feature flag.

```rust
#[cfg(any(test, feature = "test-utils"))]
impl FsBackend {
    pub fn simulate_state(&self, state: MountState);
    pub fn simulate_event(&self, event: MountEvent);
}
```

`yoke-volume` gains a `test-utils` feature. `yokectl`'s `[dev-dependencies]` enables it:

```toml
[dev-dependencies]
yoke-volume = { path = "../yoke-volume", features = ["test-utils"] }
```

Production builds never see these methods. The change is in scope for this sub-project's PR and the volume crate's spec ledger gets a note (sub-projects' specs are frozen at merge; additive feature-flagged hooks are recorded in the consumer's spec — this one).

### 14. Tests

The test surface is organized by layer. Every test is runnable on every supported host (macOS + Linux); macOS-only tests are gated by `cfg(target_os = "macos")` and skip cleanly elsewhere.

#### 14.1 `yoke-edit` unit tests

- One test per `EditOp` variant: validate-only success, validate+apply success, validate failure for each `EditError` variant.
- `apply` atomicity: a batch where op 3 fails returns the original profile unchanged.
- Progressive validation: `[AddSubProfile, SetBinding-in-the-just-added-subprofile]` succeeds.
- Suggestion quality: Levenshtein matches with a representative misspellings table (`"Sip Sof"` → `["Sip Soft"]`, `"Cougr Pull"` → `["Cougar Pull Threshold"]`, etc.).
- Catalog drift guard: a test that iterates `yoke_config::catalog::inputs`, `outputs`, `preferences`, `modes`, and `channels` and asserts every entry is reachable from at least one `EditOp` validation path. Failing this test means a new catalog entry shipped without a CLI command to reach it.
- WASM build smoke: `cargo build -p yoke-edit --target wasm32-unknown-unknown` in CI.

#### 14.2 `yoke-index` unit tests

- URL transformation table: one test per row of § 5.3.
- Index CSV parsing: header matching is case-insensitive; extra columns are preserved; rows with empty name skipped; rows with unparseable URL skipped with warn count.
- Cache TTL: a test that injects an `IndexClient` with `cache_ttl = Duration::from_millis(50)`, calls `list(false)` twice (returns cached) and then `list(false)` after 100ms sleep (refetches).
- Error mapping: every `IndexError` variant has a constructor test.

#### 14.3 `yoke-index` HTTP-mocked tests

`wiremock` dev-dep mocks the Google Sheets endpoints. One test per scenario:

- `list` against a synthetic index CSV with three rows.
- `resolve("Destiny 2")` returns the matching row.
- `fetch_profile(IndexEntry("Destiny 2"))` performs the index fetch then the per-row CSV fetch.
- `fetch_profile(Url(...pubhtml...))` rewrites and fetches.
- HTTP 404 → `IndexError::FetchFailed { status: 404 }`.
- Wrong content-type → `IndexError::FetchFailed` with the unexpected MIME.
- Connection refused → `IndexError::Network`.

One `#[ignore]` test fetches the real community index. CI runs it as a smoke canary on **push to `main` only** so column renames in the live sheet surface quickly without forcing network access on PRs (especially from fork contributors); see § 15. Locally it stays opt-in via the single `#[ignore]` gate — devs running `cargo test -- --ignored` (or naming the test under `--ignored`) accept the network access by doing so.

#### 14.4 `yokectl` unit tests

- `source::classify(s)` table: local path that exists → `LocalPath`; non-existent local path → falls through; `http://...` → `Url`; `https://...` → `Url`; bare ASCII name → `IndexEntry`.
- `target::classify` similar table.
- Output renderer: human vs JSON, color vs no-color, NDJSON for streams.
- Exit-code mapping function in `error.rs`: every priority row exercised in isolation.

#### 14.5 `yokectl` integration tests (per command)

Every command has at least one `assert_cmd::Command::cargo_bin("yokectl")` integration test that:

1. Constructs a tempdir, optionally seeded with fixture CSVs (inline string literals).
2. Invokes `yokectl --fake-volume <tempdir> ...` with the command under test.
3. Asserts stdout / stderr via `predicates` or `insta` snapshots.
4. Asserts exit code.

JSON output for every command is pinned by `insta` snapshots — schema changes require explicit snapshot review.

Exit-code coverage matrix: one integration test exhaustively iterates the § 8 table, triggering each root cause and asserting the code + JSON `error.code` match.

#### 14.6 `yokectl` E2E workflow tests

Distinct from per-command tests: each workflow chains commands as a user would, asserting the cumulative end state.

Required workflows:

1. **Install from index → list → show → set-binding → pull → diff.** `wiremock` serves an index plus one mock CSV. `install destiny --fake-volume $tmp` writes; `list` shows it; `show destiny` parses cleanly; `set-binding destiny Main "Sip Soft" "Button A"` mutates; `pull destiny` reads back; diff against an expected post-edit CSV.
2. **Install from URL → validate → set-preference → push (round-trip via local file) → diff.**
3. **Install from local file → no-network path.** Uses an inline fixture CSV; `install ./fixture.csv` becomes the no-network shortcut. Validates by default; `--no-validate` round-trip also exercised.
4. **Apply batch → verify atomicity.** Writes a fixture, runs `apply <name> --edits batch.json` where batch.json has 3 valid ops and 1 invalid op; asserts the file on disk is byte-identical to the pre-apply state and exit code 5.
5. **Watch event stream.** `assert_cmd` cannot drive `simulate_state` / `simulate_event` across a process boundary, so this workflow is **in-process**: the test constructs an `FsBackend` directly with the `test-utils` feature enabled, invokes the `watch` command's `run` function with that provider injected, captures NDJSON lines on a pipe, then calls `simulate_state` / `simulate_event` on the backend and asserts the expected event documents arrive within a 100ms timeout. Tests both transition events and the `state: ...` snapshot lines.
6. **Sub-profile lifecycle.** `subprofile add → set-binding → clone → rename → delete` against a fixture; the final state matches an expected reference.

#### 14.7 macOS-only tests

- `#[cfg(target_os = "macos")]` smoke test: construct `MacOsVolumeProvider`, drop. Same as `yoke-volume-macos`'s existing smoke test, kept in `yokectl` to verify the integration compiles and links.
- `#[cfg(target_os = "macos")]` `#[ignore]` real-device test gated by `YOKE_REAL_DEVICE=1`. Constructs the provider, subscribes, asserts a sensible `MountState` within 3 s. CI does not set the env var.

#### 14.8 Shell-completion generation

One test per shell asserts `yokectl completions <shell>` produces non-empty output containing a known marker (`_yokectl` for bash/zsh, `complete -c yokectl` for fish, `Register-ArgumentCompleter` for powershell, `edit:completion:arg-completer[yokectl]` for elvish). Output is not snapshotted (clap_complete output churns); marker presence is enough.

#### 14.9 Corpus tests

When `YOKE_CORPUS_DIR` points at a directory of real QuadStick CSVs (the maintainer's `../examples/` for local runs), an integration test:

1. Iterates every CSV in the corpus.
2. `install` each into a tempdir via `--fake-volume`.
3. `pull` each back.
4. Asserts byte equality with the source.

CI does not set `YOKE_CORPUS_DIR`; the corpus is local-only per the workspace convention.

### 15. CI

`yoke/.github/workflows/ci.yml` continues to be the single workflow. The existing gate `hashFiles('crates/**/Cargo.toml') != ''` is already true. Three new commands run in CI:

- `cargo build -p yoke-edit --target wasm32-unknown-unknown` — `yoke-edit` stays WASM-clean. Slots in next to the existing `yoke-config` WASM build.
- (No change to existing `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, or `cargo build --workspace`. The new crates ride those.)
- A small step generates each completion file (`bash`, `zsh`, `fish`, `powershell`, `elvish`) and asserts each is non-empty. Catches regressions in clap_complete bumps.

CI runs the community-index smoke step (above) only on push to `main` — PR runs (including from forks) stay fully offline. CI does **not** set `YOKE_REAL_DEVICE` or `YOKE_CORPUS_DIR`. All other integration coverage runs offline against fixtures and `wiremock`.

### 16. Acceptance criteria

This sub-project is done when:

1. `crates/yoke-edit/`, `crates/yoke-index/`, `crates/yokectl/` exist with the layout in § 1. Workspace `members` includes all three.
2. `cargo build --workspace` clean on macOS + Linux.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean on macOS + Linux.
4. `cargo test --workspace` clean on macOS + Linux. Integration suite uses `wiremock` for all HTTP; no test reaches the real internet.
5. `cargo build -p yoke-config --target wasm32-unknown-unknown` still passes. `cargo build -p yoke-edit --target wasm32-unknown-unknown` passes.
6. Shell-completion scripts generate non-empty output for bash, zsh, fish, powershell, elvish.
7. Exit-code mapping pinned by the § 14.5 coverage test.
8. JSON schema for every JSON-emitting command pinned by `insta` snapshots.
9. The `yoke-volume` `test-utils` feature is added with `simulate_state` / `simulate_event` on `FsBackend`. Production builds do not see it.
10. The five `yoke-volume` end-to-end integration tests from sub-project C continue to pass — no regression in the lower layer.
11. Maintainer-validated smoke pass (recorded in the PR description), against a real QuadStick on macOS:
    - `install <community-index-name>` resolves and writes.
    - `install <google-sheets-pubhtml-url>` works after URL rewrite.
    - `install <google-sheets-edit-url>` works against an anyone-with-link sheet.
    - `install <local-csv>` works without network.
    - `watch` shows mount/unmount/mode-change events as the device is reconfigured (DS3 emulation, Hori PS4 mode, back to base).
    - `set-binding`, `set-preference`, `subprofile clone` produce profiles that load on the device after reconnect.
    - `completions fish > ~/.config/fish/completions/yokectl.fish` plus a fresh shell autocompletes subcommands and flags.

## Out of scope (queued for future sub-projects)

- **E — Tauri shell:** consumes `yoke-volume`, `yoke-edit`, `yoke-index` from the host side; bridges to the Leptos frontend.
- **F — UI v2 (editor):** the same `yoke-edit::EditOp` enum drives the editor; the GUI shares the catalog-grounded validation with this CLI.
- **G — `yoke-device`:** HID over USB and serial transports. Backfills the reserved `device push-live`, `device save-to-slot`, `device read-live`, `protocol monitor` commands. Exit codes 64+ become live.
- **H — Windows port:** `yoke-volume-windows` lands; `yokectl`'s `cfg(target_os = "windows")` arm of `backend::open` is wired up.
- **I — Live device push** replaces volume-only writes with HID commands where possible; `install` learns to push live when the device is in HID mode.
- **J — Firmware flashing.** Stays out, as planned. Lives behind `yoke-firmware` once `yoke-device` is `confirmed(…)` on every relevant fact.
- **K — Distribution / packaging.** Homebrew tap, MSI installer, signed binaries, completion-script installers. Man pages from `clap_mangen` ship here.
- **Dynamic completion** of profile names and catalog values. Hidden `--complete <shell> <args>...` command that the static scripts call back into.

## Forward references

- `yoke-edit::EditOp` is the contract the Leptos UI in sub-project F will consume. The JSON tagging is `kebab-case` so the wire shape stays readable when sub-project E proxies edits through Tauri IPC.
- `yoke-index::COMMUNITY_INDEX_URL` is the canonical pin. Updating it requires a `yoke-index` release; this is deliberate so users do not silently follow a redirect.
- The `device push-live` / `device save-to-slot` / `device read-live` / `protocol monitor` names are reserved. Sub-project G implements them without changing the grammar.
- The `yoke-volume` `test-utils` feature is the first cross-crate test-only addition. Future sub-projects with similar needs should follow the same pattern (feature-gated, additive, recorded in the consumer's spec) rather than mutating shipped specs.
