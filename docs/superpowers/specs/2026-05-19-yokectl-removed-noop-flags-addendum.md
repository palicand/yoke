# yokectl: remove no-op `watch --include-poll` and `pull --raw` (addendum)

**Status:** accepted (2026-05-19)
**Supersedes (partially):** [2026-05-18-yokectl-design.md](2026-05-18-yokectl-design.md) §§ 3.1, 3.2, 10
**Related:** [2026-05-19-yokectl-docs-addendum.md](2026-05-19-yokectl-docs-addendum.md) § 1 (which paraphrases the now-removed long_about text)

## Context

The parent yokectl design names two flags that landed as user-visible no-ops:

- `watch --include-poll` (parent § 3.1 row "watch", § 10 row 4) — intended to surface `MacOsVolumeProvider` poll-tick events alongside public `MountEvent`s. The backend never gained a poll-tick stream, so the flag's `clap` arg was accepted, the handler took it as `_include_poll`, and threw it away.
- `pull --raw` (parent § 3.2 row "pull", § 10 row 2) — intended to skip re-serialization. `commands::profile::run_pull` already writes `provider.read_profile(...)` bytes through `std::fs::write` verbatim — i.e. it is *always* byte-identical — so `--raw` had no effect.

A user-visible flag that does nothing is a quiet documentation lie: `--help` advertises a capability, scripts may pass it expecting behavior, and the code carries dead arguments.

## Decision

Drop both flags from the v1 CLI surface. Specifically:

| Site | Before | After |
|---|---|---|
| `cli.rs::Commands::Watch` | `Watch { include_poll: bool }` plus `long_about` mentioning `--include-poll` | unit variant `Watch`; `long_about` trimmed |
| `cli.rs::Commands::Pull` | `Pull { name, dest, raw: bool }` | `Pull { name, dest }` |
| `commands::profile::run_pull` | takes `_raw: bool` | argument removed |
| `commands::watch::run` | takes `_include_poll: bool` | argument removed |
| `tests/snapshots/help_snapshots__help_text_snapshots@{watch,pull}.snap` | listed the flags | regenerated |

`show --raw` is **not** affected — it has a real implementation (`commands::profile::run_show` writes verbatim bytes to stdout when `raw` is set) and the spec's documented behavior matches.

## Reinstatement criteria

Either flag returns to the CLI only after the underlying capability ships:

- `watch --include-poll` returns once `VolumeProvider` (or `MacOsVolumeProvider` directly) exposes a poll-tick stream and `commands::watch::run` actually consumes it.
- `pull --raw` returns only if `pull` ever gains a non-raw mode (e.g., reformat on write). Today there is none, so `--raw` would still be vacuous.

When reinstating, restore the parent spec's wording rather than re-deriving it.

## Out of scope

The parent spec table text in § 3.1/§ 3.2/§ 10 is left intact as a historical record; this addendum is the authoritative override.
