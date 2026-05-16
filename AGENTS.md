# AGENTS.md

Conventions for any agent (human or LLM-driven) working in this repository.
Read top-to-bottom before opening files.

## What Yoke is

Configuration software for the QuadStick. All-Rust. Tauri 2 desktop shell;
Leptos WASM frontend; CLI for programmatic and agentic workflows.
macOS first, Windows planned. See [`README.md`](./README.md) for the
user-facing summary and [`docs/superpowers/specs/`](./docs/superpowers/specs)
for the architectural decisions.

## Repository map

```
yoke/
├── Cargo.toml                # virtual workspace (no members yet)
├── rust-toolchain.toml       # Rust channel + components + wasm target
├── flake.nix                 # Nix devShell (reads rust-toolchain.toml)
├── flake.lock                # pinned flake inputs
├── .envrc                    # direnv: use flake
├── README.md                 # user-facing
├── AGENTS.md                 # this file
├── LICENSE
├── .github/workflows/ci.yml  # CI inside the devShell
└── docs/
    ├── README.md             # docs index
    └── superpowers/
        └── specs/            # architectural decisions (committed)
                              # plans/ is git-ignored, local-only
```

The QuadStick wire-protocol notes are deliberately **not** in this
repo. They live in the maintainer's local notes / Obsidian vault while
many facts are still `inferred` or `unknown`. Sections will be
promoted into the repo (likely as `crates/yoke-device/PROTOCOL.md`)
once each fact is confirmed.

Crates land under `crates/` as the sub-projects that need them are
implemented. The intended eventual layout is documented in the scaffold
spec.

## Build, run, test

All commands assume the Nix devShell is active (direnv handles this on
`cd`) or rustup + the toolchain from `rust-toolchain.toml` is available.

| Command | Effect |
|---|---|
| `cargo metadata --no-deps` | Parse the workspace manifest. Works even when `members = []`. CI runs this on every push as the workspace integrity gate. |
| `cargo check --workspace` | Type-check every workspace member. **Errors on an empty workspace** — only useful once the first crate lands. |
| `cargo fmt --all --check` | Format check. Same caveat as above. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint with warnings-as-errors. Same caveat. |
| `cargo test --workspace` | Run all unit and integration tests. Same caveat. |
| `cargo tauri dev` | Run the desktop app in dev mode (once `yoke-tauri` exists). |
| `trunk serve` (inside `crates/yoke-ui/`) | Run the Leptos UI as a regular browser app for development (once `yoke-ui` exists). |

## House rules

1. **No emojis.** Never in code, comments, docs, commit messages, PR
   bodies, or chat output, unless the user has used an emoji in the
   current conversation or explicitly asked for one. This is a hard rule.
2. **No comments except WHY-comments.** Do not explain *what* code does —
   well-named identifiers do that. Do not reference the current task,
   fix, or callers in code (that belongs in the PR description and rots
   over time). Only add a comment when the *why* would not be obvious
   from reading the code: a hidden constraint, a subtle invariant, a
   workaround for a specific bug.
3. **Errors:** `thiserror` in library crates, `anyhow` in binary crates.
4. **Logging:** `tracing` + `tracing-subscriber`.
5. **Dependencies:** add and remove with `cargo add` / `cargo remove`.
   Never hand-edit a crate's `[dependencies]` table. Workspace-level
   metadata edits in the root `Cargo.toml` are the only exception.
6. **Commits:** title line only, no body. Commits are squashed on merge;
   commit titles use Conventional Commits prefixes (`feat`, `fix`,
   `docs`, `chore`, `refactor`, `test`).

## Spec / plan workflow

1. **Brainstorm** new sub-projects into a spec under
   `docs/superpowers/specs/YYYY-MM-DD-<slug>-design.md`. Specs are
   **committed**; they are the authoritative architectural record.
2. **Write a plan** for the spec into
   `docs/superpowers/plans/YYYY-MM-DD-<slug>.md`. Plans are bite-sized
   tasks an agent can execute step-by-step. Plans are **local-only
   working artifacts and are not committed** — `docs/superpowers/plans/`
   is git-ignored. The reasoning: a plan is ephemeral context for one
   execution pass; once the implementation lands, the spec carries the
   decisions and the code carries the result.
3. **Implement** the plan, ideally task-by-task with subagents per
   task. Frequent commits.

## Parallel-agent coordination

When a task spans multiple crates or has independent sub-tasks, use the
host platform's parallel-agent primitive instead of serializing the work
through a single agent. Independent crates typically warrant independent
agents.

- **Claude Code:** `TeamCreate` for coordination; the `Explore` subagent
  type for read-only investigations.
- **Other platforms:** the equivalent multi-agent or parallel-task call.

If your platform has no such primitive, fall back to sequential work and
note it in the PR description.

## UI development substrate

`yoke-ui` (once it exists) must remain runnable as a standalone browser
app via `trunk serve` against a mock IPC backend, in addition to running
inside the Tauri shell. This is what lets agents that cannot see a native
desktop window iterate on the UI through a regular browser. The Tauri
shell is the production wrapper, not the development substrate.

## Platform prerequisites

- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
  Required for the linker, system headers, and the WebKit framework
  Tauri's webview uses. Not provided by the flake — outside nixpkgs.
- **Linux** (when the Linux port begins): `webkit2gtk-4.1` and
  `libayatana-appindicator`. A commented-out block in `flake.nix` is
  ready to enable.
- **Windows** (when the Windows port begins): WebView2 runtime
  (ships with current Windows 11) and Visual Studio Build Tools. Not a
  Nix target.

## Non-Nix contributors

- macOS: `xcode-select --install`, then install rustup. The committed
  `rust-toolchain.toml` auto-fetches the channel, components, and the
  `wasm32-unknown-unknown` target on first `cargo` invocation.
  Then: `cargo install trunk tauri-cli`.
- Windows: instructions land when sub-project H begins.

## Fixtures

Example QuadStick config CSVs live in the parent workspace at
`../examples/` (one level above the Yoke checkout). Once `yoke-config`
exists (sub-project B), copies for unit tests will be checked in at
`fixtures/csv/`. The QuadStick volume mount itself may or may not be
present at any time — `enable_DS3_emulation` mode controls exposure and
macOS enumeration is racy. Refer to the maintainer's local wire-protocol
notes for the underlying USB-level details (those notes are not in this
repo yet — see the repository map for why).

## What lives where (decisions reference)

- The Yoke name: `design_handoff_quadstick_config/NAMING.md` in the
  parent workspace.
- The visual design reference: `design_handoff_quadstick_config/` in the
  parent workspace. Treat as Figma-equivalent — the React JSX in it is
  not a port target.
- Vocabulary catalog source of truth (until ported to Rust):
  `design_handoff_quadstick_config/src/data.js`.
- Original Windows manager source (primary protocol source):
  <https://github.com/fdavison/QMP-4>.
- macOS fork (cross-check for portability):
  <https://github.com/cchriskeach/QMP-4>.
