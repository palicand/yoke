# Yoke documentation

## Architecture and decisions

- [`superpowers/specs/`](./superpowers/specs) — architectural decisions
  produced by brainstorming sessions. Authoritative source for "why
  Yoke is shaped this way". Each spec is dated and named for the
  sub-project it covers.
- `superpowers/plans/` (not present on disk by default) —
  implementation plans derived from the specs. These are local-only
  working artifacts during sub-project execution and are git-ignored;
  they do not appear in the committed tree.

## Device

The QuadStick wire-protocol notes are **not** in this repo. They live
in the maintainer's local Obsidian vault while many facts are still
`inferred` or `unknown`; promoting them to git would make speculation
look like ground truth. Sections will be moved into the repo (likely
as `crates/yoke-device/PROTOCOL.md`) once each fact is confirmed.

Authoritative external sources for the device wire protocol:

- [QMP-4](https://github.com/fdavison/QMP-4) — Fred Davidson's
  upstream Windows manager, most feature-complete.
- [QMP-mac](https://github.com/cchriskeach/QMP-4) — Chris Keach's
  macOS fork; identifies which behaviors are platform-portable.

## Reading order for new contributors

1. The repo-root [`README.md`](../README.md) — what Yoke is.
2. [`AGENTS.md`](../AGENTS.md) — house rules and contributor flow.
3. The most recent spec in `superpowers/specs/` — current architectural
   state.
