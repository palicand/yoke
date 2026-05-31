# yoke-edit binding ops (`add` / `update` / `clear`) + `yokectl` parity

**Date:** 2026-05-30
**Stage:** F (enabling slice — ships ahead of the GUI editor)
**Status:** proposed
**Predecessors:** [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)
**Successor:** the forthcoming Stage F GUI editor spec (a separate sub-project/worktree) builds on these ops.

## Goal

Give `yoke-edit` a complete, **corruption-safe** binding-edit vocabulary — create, modify (including the modifier), and delete a binding — and expose it through `yokectl` with full fidelity (dedicated commands, batch `apply`, tab-completion, introspection, generated docs, readback). Replace the previous `set-binding` / `set-modifier` shapes, which addressed a binding by its input alone and silently mutated the first match.

This slice is self-contained and independently useful from the CLI; it merges before any GUI work.

## Why (and why it is critical, not cosmetic)

A QuadStick profile is frequently the user's *only* means of operating the computer. **Any edit path that can silently corrupt or mis-target a binding is a critical bug**, on par with bricking a keyboard for an able-bodied user: recovery may require a reset or a caregiver. So the bar for these ops is not "usually right" — it is "never silently wrong; refuse when ambiguous."

The previous design failed that bar. A binding row is `output, modifier, input`, and **input does not uniquely identify a row** — one input legitimately drives several outputs (a chord). Real corpus proof: `examples/default.csv` binds input `lip` to *both* `kb_left_gui [normal]` and `kb_left_shift [delay_on 1000]` in one sub-profile. The old `apply_set_binding` / (proposed) `apply_set_modifier` resolved by input and mutated the *first* matching row, so `set-modifier … lip …` would rewrite the wrong binding and report success. That is the corruption class above.

The GUI editor will route every edit through `yoke_edit::apply` so its undo/redo op-log re-derives deterministically; these edits must therefore be `EditOp`s, not ad-hoc mutations. Landing the ops (and their CLI surface) first keeps that sub-project focused on UI.

## Binding identity model

A binding row is `(input, modifier) → output`. The **`(input, modifier)` pair is the key** and maps to exactly one output; `(input, output)` is *not* unique (the same output may be produced by the same input under different modifiers). Every op is defined against this key, and any operation that cannot resolve to a single row **errors instead of guessing**.

The user chooses intent explicitly (REST-like): `add` is POST, `update` is PUT.

**`add-binding` (POST)** — append a new row. Error iff a row already exists for this `(input, modifier)` (it would create a second output for one key). A duplicate `(input, output)` with a *different* modifier, or a parallel output for the same input, is allowed (worst case is a redundant row, never a silent overwrite).

| input | modifier | output | action |
|---|---|---|---|
| new | new | new | add |
| exists | new | new | add |
| exists | exists | * | **error** (`BindingExists`) |
| exists | new | exists | add (duplicate output, distinct modifier) |

("exists" is per-input: *modifier exists* = a row already has this `(input, modifier)`; *output exists* = a row already has this `(input, output)`.)

**`update-binding` (PUT)** — mutate the single existing row, anchored by whichever of `(input, modifier)` / `(input, output)` is present; change the other field.

| (input,modifier) | (input,output) | action |
|---|---|---|
| exists (unique) | — | change that row's **output** |
| — | exists (unique) | change that row's **modifier** |
| exact triple present | | noop |
| neither | | **error** (`BindingNotFound`) |
| both, different rows / `(input,output)` multi-match | | **error** (`AmbiguousBinding`) |

`(input, modifier)` is unique by the add invariant, so the modifier-anchored path is always unambiguous; `(input, output)` may match multiple rows, so that path refuses on a multi-match rather than picking one.

**`clear-binding` (DELETE)** — `--modifier` omitted removes *every* row for the input; `--modifier M` removes only the unique `(input, M)` row. `BindingNotFound` if nothing matched (catalog-valid-but-unbound input included; a genuinely unparseable input is still `UnknownInput`).

## Changes

### 1. `yoke-config` — modifier keywords + per-variant `keyword()`

[`Modifier`](../../../crates/yoke-config/src/catalog/modifiers.rs) gains `KEYWORDS: &'static [&'static str]` (the 14 leading tokens: `normal, toggle, delay_on, delay_off, greater_than, less_than, repeat, pulse, duty, force_off, delayed_latch, tap, increment_value, decrement_value`; `Unknown` excluded) for the completer, introspection, and "did you mean" suggestions. It also gains `const fn keyword(&self) -> Option<&'static str>` — an **exhaustive match** so a new variant cannot compile without declaring its keyword; a test ties `KEYWORDS` ↔ `from_csv` ↔ `keyword()` so the three cannot silently disagree. (Full reverse-enforcement — "every variant appears in `KEYWORDS`" — would need an enum-iteration dependency such as `strum`; not pulled in for a non-corruption, completer-only gap.)

### 2. `yoke-edit` — `AddBinding` / `UpdateBinding`, `ClearBinding { modifier }`

```rust
EditOp::AddBinding    { sub_profile, input, output, modifier: Option<String> } // None => "normal"
EditOp::UpdateBinding { sub_profile, input, output, modifier: String }
EditOp::ClearBinding  { sub_profile, input, modifier: Option<String> }
```

`serde(tag = "op", rename_all = "kebab-case")` makes these reachable in batch `apply` as `{"op":"add-binding",…}` etc. with no extra wiring. `SetBinding` and `SetModifier` are **removed** (pre-1.0, no aliases).

Errors (in [`error.rs`](../../../crates/yoke-edit/src/error.rs)): `BindingExists { sub_profile, input, modifier, output }`, `BindingNotFound { sub_profile, input }`, `AmbiguousBinding { sub_profile, input, output }`. `NoBindingForInput` is removed. Modifier parsing reuses `parse_modifier`, which splits the failure on the **leading keyword token**: an unknown keyword yields `UnknownModifier { modifier, suggestions }` scored against `KEYWORDS`, while a *recognized* keyword carrying bad/extra arguments (`delay_on abc`) yields `InvalidModifierArguments { keyword, modifier }` — reporting the argument fault directly instead of echoing the keyword back as its own suggestion. `clear-binding` on a valid-but-unbound input now returns `BindingNotFound` rather than the old `UnknownInput { suggestions: [] }` fudge.

### 3. `yokectl` — commands, completion, introspection, docs, readback

- **Commands** `add-binding` / `update-binding` / `clear-binding` (in [`cli.rs`](../../../crates/yokectl/src/cli.rs), handlers in [`commands/edit.rs`](../../../crates/yokectl/src/commands/edit.rs)). `modifier` is a `--modifier` flag: optional on `add` (default `normal`) and `clear` (scope), required on `update`. E.g. `yokectl add-binding destiny Main lip_soft kb_a --modifier "delay_on 250"`.
- **Completion** — `CatalogKind::Modifier` (already present) attached to every `--modifier`.
- **Introspection** — `catalog modifiers` (`CatalogCmd::Modifiers` + `run_modifiers`), the dimension that lacked introspection; human / json / ndjson, pinned by a `catalog_modifiers_json_snapshot`.
- **Docs** — generated from the clap tree; output now lists `add-binding` / `update-binding` / `clear-binding` and `catalog modifiers`.
- **Readback** — `yokectl bindings` surfaces the modifier (`GroupedBinding` in [`commands/view.rs`](../../../crates/yokectl/src/commands/view.rs)): JSON always carries `modifier`; the human view appends `[<modifier>]` only when it is not the default, derived from `Modifier::Normal.to_csv()` rather than a hardcoded literal.
- **Error envelopes** — `edit-binding-exists`, `edit-binding-not-found`, `edit-ambiguous-binding`, `edit-invalid-modifier-arguments` in [`error.rs`](../../../crates/yokectl/src/error.rs).

### 4. `AGENTS.md`

The standing "public `EditOp` change carries its CLI in the same slice" rule stays; this slice continues to satisfy it.

## Out of scope

No changes to `yoke-volume` or `yoke-gui`. The redundant-duplicate *advisory* on `add` (warn when a parallel `(input, output)` row is created) is left to the GUI/CLI presentation layer; `apply` stays pure and simply allows it.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `yoke-config` | `KEYWORDS` lists every typed modifier; `keyword()` ↔ `from_csv` ↔ `KEYWORDS` consistency; `Unknown` has no keyword | host `cargo test` |
| `yoke-edit` | every truth-table cell: add create / parallel / duplicate-output / conflict; update output-anchored / modifier-anchored / noop / not-found / ambiguous (multi-match and split-anchor); clear all / scoped / not-found / unparseable | host `cargo test` |
| `yokectl` | `add`/`update`/`clear` parse and apply (good + bad exit codes); `catalog modifiers`; completion; `bindings` readback | CLI test harness |
| `yokectl` fuzz | the three ops (single + batch) and `catalog modifiers` in the proptest strategies, now seeded with a **`Main` sub-profile carrying a binding** so the success and not-found branches are actually exercised (previously every sub-profile-scoped op bailed at `SubProfileNotFound`) | `cargo test -p yokectl --test property` |

All standard gates stay green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `wasm32` build of `yoke-config` (`KEYWORDS` / `keyword()` are `const`/pure, dual-target-safe).

## References

- Op engine: [`yoke-edit`](../../../crates/yoke-edit) (`op.rs`, `apply.rs`, `suggest.rs`, `error.rs`).
- CLI: [`yokectl`](../../../crates/yokectl) (`cli.rs`, `commands/edit.rs`, `commands/catalog.rs`, `completion/catalog.rs`, `commands/view.rs`).
- Catalog: [`Modifier`](../../../crates/yoke-config/src/catalog/modifiers.rs), [`Input`](../../../crates/yoke-config/src/catalog/inputs.rs).
- Corpus evidence for non-unique input: `examples/default.csv` (`lip` → two outputs).
- CLI design + addenda: [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md) and the 2026-05-19 / 2026-05-25 addenda.
