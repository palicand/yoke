# yokectl — docs addendum

- **Date:** 2026-05-19
- **Status:** Approved, ready for implementation plan
- **Parent spec:** [2026-05-18-yokectl-design.md](./2026-05-18-yokectl-design.md)
- **Sub-project ID:** D (`yokectl`)

## Context

The yokectl sub-project shipped with structurally complete clap dispatch but every `#[command]` and `#[arg]` is undescribed — `yokectl --help` lists names with no `about` lines and no flag descriptions. The parent spec also deferred man-page generation to the distribution sub-project (§ 16.K), even though `clap_mangen` is already a yokectl dependency (parent § 1, "used in tests; binary artifact is not yet shipped").

This addendum closes both gaps in the current sub-project so the CLI is self-documenting before D is considered done.

## Goals

1. Every command and every flag has a one-line `about` (or `help` for args) suitable for `-h`. A subset of subcommands that benefit from examples or extra detail gain `long_about`.
2. A `yokectl docs --format man|md --out <dir>` subcommand emits, on demand, a complete man-page tree and a single markdown reference, mirroring the established `yokectl completions <shell>` pattern.
3. CI generates both artifact sets each run and asserts non-empty output for a known marker, the same gate already used for completions.
4. HTML is not produced in this round. The generated markdown is human-readable on GitHub and crates.io; richer HTML stays queued for sub-project K (where `mandoc -Thtml` over the man pages is a one-line build step).

## Non-goals

- **No standalone HTML renderer.** No `pulldown-cmark`, no static-site generator, no templating in this round. If markdown is not enough, we pivot.
- **No installed artifacts.** Man pages and markdown are generated into a directory the user picks; this sub-project does not install them to `/usr/local/share/man` or anywhere else. Distribution lives in K.
- **No man-page content beyond what clap emits.** `clap_mangen` derives sections from the same metadata that drives `--help`; we do not hand-author additional `EXAMPLES` or `SEE ALSO` sections in this round. They can be appended later via post-processing if useful.
- **No dynamic completion or man-page-for-each-shell.** Out of scope, same as in the parent spec.

## Design

### 1. Help-text coverage

Every `Commands::*` variant gets a `#[command(about = "…")]`. Every `#[arg]` gets a `help = "…"`. The texts come from the parent spec's command tables (§§ 3.1–3.10) and are short — one line each, no trailing period (clap convention).

Where the parent spec already names a "long" semantic (e.g., `install` auto-classification rules, `watch --include-poll` JSON-only constraint, `delete --force` interaction with `--json`), the variant gets a `long_about` that paraphrases the relevant spec sentence. The intent is that `yokectl install --help` is enough for an agent to use it correctly without opening the spec.

Global flags on `Cli` (`--fake-volume`, `--json`, `-v`, `--no-color`) also get `help` strings drawn from parent § 3 top-level table.

### 2. The `docs` subcommand

```text
yokectl docs --format <FORMAT> --out <DIR>
```

| Flag | Effect |
|---|---|
| `--format <man\|md>` | Required. `man` writes a `roff(7)` man-page tree; `md` writes one markdown file. |
| `--out <DIR>` | Required. Directory is created if missing. |

Output layout under `<DIR>`:

```text
<DIR>/
├── man/                       # only when --format man
│   ├── yokectl.1
│   ├── yokectl-device.1
│   ├── yokectl-watch.1
│   ├── yokectl-list.1
│   ├── …                      # one .1 per leaf subcommand
│   ├── yokectl-subprofile.1   # the parent for the nested group
│   ├── yokectl-subprofile-add.1
│   └── …
└── markdown/                  # only when --format md
    └── yokectl.md             # single document, hierarchical headings
```

Running with `--format man --out <dir>` then with `--format md --out <dir>` leaves both subtrees side by side; the command is idempotent on re-run (existing files are overwritten, no garbage accumulates).

#### 2.1 Generation strategy

Man pages: `clap_mangen::Man` walks the `Cli` clap `Command` tree recursively. For each node we render a `.1` file named with the dashed path (`yokectl-subprofile-add.1`). Section 1 ("user commands") is correct for every page.

Markdown: implemented in-crate as a small recursive walker over the same clap `Command` tree, emitting `#`, `##`, `###` headings per nesting level, a `Usage:` block from clap's rendered usage line, an `Options:` block built by iterating `cmd.get_arguments()`, and the same `about`/`long_about` text. Producing markdown in-crate keeps the dependency footprint small (no `clap-markdown` crate); the renderer is ~120 lines of pure-Rust formatting and is unit-tested.

#### 2.2 Surface placement

The subcommand lives next to `completions` in `crates/yokectl/src/commands/`. New file: `docs.rs`. Wired into `Commands::Docs { format, out }` in `cli.rs`. Tests live in the same module plus an integration test that asserts:

- `yokectl docs --format man --out $tmp` produces `yokectl.1` with `\.TH "YOKECTL" "1"` near the top.
- `yokectl docs --format md --out $tmp` produces `yokectl.md` starting with `# yokectl` and containing one `## ` heading per top-level subcommand.

### 3. Error handling

- Missing `--out` directory: created via `std::fs::create_dir_all`. Failure → exit code 6 (I/O), per the parent § 8 mapping.
- Invalid `--format` value: clap parse failure → exit code 2.
- Write failure for any individual file: short-circuit; exit code 6 with the failing path in the error chain.

No new exit codes; the existing mapping covers this command.

### 4. CI

The existing completions step expands to a docs step too. The CI workflow runs:

```bash
mkdir -p target/ci-docs
cargo run --quiet -p yokectl -- docs --format man --out target/ci-docs
cargo run --quiet -p yokectl -- docs --format md  --out target/ci-docs
test -s target/ci-docs/man/yokectl.1
test -s target/ci-docs/markdown/yokectl.md
```

Both man and markdown gates use the same "non-empty output" criterion the completions step already uses. No new tools (`mandoc`, `pandoc`) added to the Nix devShell. The generated tree is not uploaded as an artifact in this round; K will pick it up.

### 5. Amendment to the parent spec

Parent spec § 16.K reads:

> **K — Distribution / packaging.** Homebrew tap, MSI installer, signed binaries, completion-script installers. **Man pages from `clap_mangen` ship here.**

Replace the bold sentence with:

> Man-page and markdown artifacts are produced by `yokectl docs` (see [2026-05-19-yokectl-docs-addendum.md](./2026-05-19-yokectl-docs-addendum.md)); K installs them to platform-appropriate locations and ships richer HTML if needed (e.g., `mandoc -Thtml` over the man pages).

This is the only edit to the parent spec; it is recorded here so the parent stays the canonical design and this addendum stays the canonical record of the change.

## Tests

- **Help-text smoke**: a `yokectl --help` snapshot test (via `insta`) pins the top-level summary; one per leaf subcommand for `<cmd> --help`. Snapshot churn on copy edits is acceptable — review on change.
- **`docs` integration test (man)**: `yokectl docs --format man --out $tmp`; assert `yokectl.1` exists, starts with `.TH`, and that one `.1` exists per leaf subcommand (count matches `Commands::*` plus nested `subprofile`/`index`/`catalog` arms).
- **`docs` integration test (md)**: `yokectl docs --format md --out $tmp`; assert `yokectl.md` exists, starts with `# yokectl`, contains a `## ` heading for every top-level subcommand, and contains an `### ` heading for every leaf of the three nested groups.
- **Idempotency**: run each format twice into the same `--out`; assert the byte-for-byte content is identical between runs.
- **Markdown renderer unit tests**: small Cargo workspace fixture with two subcommands; assert nesting and heading levels.

## Acceptance criteria

This addendum is done when:

1. Every `Commands` variant and every `#[arg]` (global and per-command) carries an `about` / `help` string. `yokectl --help` and `yokectl <any-command> --help` produce non-empty descriptions.
2. `yokectl docs --format man --out <dir>` and `yokectl docs --format md --out <dir>` both work and pass the integration tests above.
3. The CI workflow has the new docs step and it passes.
4. The parent spec § 16.K bullet is amended in the same PR.
5. `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, and `cargo test --workspace` all clean on macOS + Linux.

## Out of scope (queued for sub-project K)

- HTML rendering (likely via `mandoc -Thtml` against the generated `.1` files).
- Installation of artifacts to `/usr/local/share/man`, `share/doc`, or platform equivalents.
- Hand-authored `EXAMPLES` / `SEE ALSO` blocks beyond what clap emits.
- A Homebrew formula / MSI / .deb that bundles the docs.

## Forward references

- The `yokectl docs` subcommand pattern (single binary emits its own static artifacts) is a candidate template for any future "generate something for packagers" command. Completions already follow it; docs will be the second.
- If markdown turns out to be insufficient — agreed pivot point with the user — we move HTML generation forward into this sub-project rather than waiting for K. The pivot most likely lands as an additional `--format html` variant that runs `mandoc -Thtml` over the generated man pages.
