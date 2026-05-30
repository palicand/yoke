# yoke-edit `SetModifier` op + `yokectl` parity

**Date:** 2026-05-30
**Stage:** F (enabling slice — ships ahead of the GUI editor)
**Status:** proposed
**Predecessors:** [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)
**Successor:** the forthcoming Stage F GUI editor spec (a separate sub-project/worktree) builds on this op.

## Goal

Add a first-class **modifier** edit operation to `yoke-edit` and expose it through `yokectl` with the same fidelity as the other edit verbs (dedicated command, batch `apply` support, tab-completion, introspection, generated docs). Adopt and codify the standing rule that a public-facing `EditOp` change carries its CLI change in the same slice.

This slice is self-contained and independently useful from the CLI; it merges before any GUI work.

## Why

The current [`EditOp`](../../../crates/yoke-edit/src/op.rs) set has no way to set a binding's modifier. [`apply_set_binding`](../../../crates/yoke-edit/src/apply.rs) sets only the output — forcing `Modifier::Normal` on a freshly created binding and preserving the existing modifier on replace. Modifiers (`delay_on`, `toggle`, `greater_than`, `repeat`, `pulse`, …) are a core part of QuadStick bindings and currently cannot be changed programmatically at all.

The GUI editor will route every edit through `yoke_edit::apply` so its undo/redo op-log re-derives deterministically; a modifier edit therefore must be an `EditOp`, not an ad-hoc mutation. Landing the op (and its CLI surface) first keeps that sub-project focused on UI.

## Standing convention (codified here)

**A change that adds or modifies a public-facing edit operation (`yoke_edit::op::EditOp`) must update the `yokectl` CLI in the same slice** — the op gets its command, batch-`apply` coverage, shell-completion entry, introspection where the dimension warrants it, and generated docs. An op present in the library but absent from the CLI is a silent capability gap (the CLI is a first-class consumer for agentic/headless flows) and a drift source.

This slice adds the rule to [`AGENTS.md`](../../../AGENTS.md) house rules and is itself the first application of it.

## Changes

The op needs a catalog primitive that does not exist yet, so the slice spans three crates.

### 1. `yoke-config` — `Modifier` keyword enumerator

[`Modifier`](../../../crates/yoke-config/src/catalog/modifiers.rs) exposes only `from_csv` / `to_csv`; there is no list of modifier keywords. Both the CLI completer and the introspection command need one, and `apply` needs it to produce "did you mean" suggestions on a misspelled modifier (mirroring how `parse_input` / `parse_output` already suggest).

Add an enumerator of the modifier **keywords** (the leading token of the csv phrase): `normal, toggle, delay_on, delay_off, greater_than, less_than, repeat, pulse, duty, force_off, delayed_latch, tap, increment_value, decrement_value`. Exact name/shape (`Modifier::KEYWORDS: &[&str]` vs `fn keywords()`) is pinned at implementation; it is keyword-only because a full modifier value is a `keyword [args]` phrase, not a closed set. `Unknown` is excluded.

### 2. `yoke-edit` — `EditOp::SetModifier`

```rust
EditOp::SetModifier { sub_profile: String, input: String, modifier: String }
```

`serde(tag = "op", rename_all = "kebab-case")` is already derived on `EditOp`, so the new variant is automatically reachable through the `apply` batch command as `{"op": "set-modifier", ...}` — no extra wiring for batches.

`apply_set_modifier` semantics:

- Resolve `sub_profile` by name (`SubProfileNotFound` otherwise) — same helper as the other ops.
- Parse `input` via `Input::from_csv`; reject `Input::Unknown` with `EditError::UnknownInput { suggestions }` (reuse `parse_input`).
- Parse `modifier` via `Modifier::from_csv`; reject a `Modifier::Unknown` keyword with a new `EditError::UnknownModifier { modifier, suggestions }` built from the §1 keyword list via the existing [`suggest`](../../../crates/yoke-edit/src/suggest.rs) helper. (A malformed *argument* on a known keyword round-trips to `Modifier::Unknown` today; this path reports it as an unknown modifier — acceptable for now.)
- Find the binding for `input` in that sub-profile; if none exists, error (`set the output first`) — a new `EditError` variant or the existing unknown-input shape, decided at implementation. Modifier editing does not create bindings.
- Replace only the binding's `modifier` field; output, input, and comment are untouched.

Unit tests alongside the existing op tests: replace-on-existing, error on unbound input, error + suggestion on unknown modifier keyword, and round-trip through `Modifier::from_csv` for an argument-carrying modifier (e.g. `delay_on 250`).

### 3. `yokectl` — command, completion, introspection, docs

- **Command** `SetModifier { target, sub_profile, input, modifier }`, mirroring [`SetBinding`](../../../crates/yokectl/src/cli.rs): `yokectl set-modifier <target> <sub-profile> <input> <modifier>`, e.g. `yokectl set-modifier destiny Main lip_soft "delay_on 250"`. The `modifier` argument is a single (shell-quoted) phrase, consistent with the `EditOp` string and how modifiers appear in the CSV. Handler in [`commands/edit.rs`](../../../crates/yokectl/src/commands/edit.rs) building `EditOp::SetModifier`, reusing the existing target-load → apply → write path and exit codes.
- **Completion** — add `CatalogKind::Modifier` to the [completion catalog](../../../crates/yokectl/src/completion/catalog.rs), backed by the §1 keyword enumerator, and attach `CatalogValueCompleter(CatalogKind::Modifier)` to the `modifier` argument (input keeps `CatalogKind::Input`). Completion offers the keyword; numeric arguments are typed by hand.
- **Introspection** — add `catalog modifiers` (`CatalogCmd::Modifiers` + `run_modifiers` in [`commands/catalog.rs`](../../../crates/yokectl/src/commands/catalog.rs)), filling the one catalog dimension that lacks introspection (inputs/outputs/preferences/modes/channels already have it). Reuses the §1 enumerator; honors the existing `human` / `json` / `ndjson` output formats.
- **Docs** — generated from the clap tree by the existing `docs` subcommand; no manual doc edits, but the generated output now includes `set-modifier` and `catalog modifiers`.

### 4. `AGENTS.md`

Add the §"Standing convention" rule to the house-rules list.

## Out of scope

No changes to `yoke-volume` or `yoke-gui` in this slice.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `yoke-config` | keyword enumerator lists every known modifier; round-trips against `from_csv`/`to_csv` | host `cargo test` |
| `yoke-edit` | `SetModifier` apply: replace on existing binding, error on unbound input, error + suggestion on unknown modifier, arg-carrying round-trip; batch `apply` reaches it via serde | host `cargo test` |
| `yokectl` | `set-modifier` parses and applies (good + bad input/modifier exit codes); `catalog modifiers` lists in each output format; completion yields modifier keywords | existing CLI test harness |

All standard gates stay green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `wasm32` build of `yoke-config` (the keyword enumerator is `const`/pure, dual-target-safe).

## References

- Op engine: [`yoke-edit`](../../../crates/yoke-edit) (`op.rs`, `apply.rs`, `suggest.rs`, `error.rs`).
- CLI: [`yokectl`](../../../crates/yokectl) (`cli.rs`, `commands/edit.rs`, `commands/catalog.rs`, `completion/catalog.rs`).
- Catalog: [`Modifier`](../../../crates/yoke-config/src/catalog/modifiers.rs), [`Input`](../../../crates/yoke-config/src/catalog/inputs.rs).
- CLI design + addenda: [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md) and the 2026-05-19 / 2026-05-25 addenda.
