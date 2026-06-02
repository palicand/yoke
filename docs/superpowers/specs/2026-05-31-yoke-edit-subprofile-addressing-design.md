# yoke-edit sub-profile addressing by index + `yokectl` parity

**Date:** 2026-05-31
**Stage:** F (enabling slice — ships ahead of the GUI editor)
**Status:** proposed
**Predecessors:** [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md), [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)
**Supersedes (in part):** the sub-profile-addressing portion of [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md) — that slice addressed every sub-profile-scoped op by `profile_name` (a `String`). Its `(input, modifier)` binding-identity model is unchanged and remains authoritative.
**Successor:** [`2026-05-30-yoke-gui-editor-design.md`](2026-05-30-yoke-gui-editor-design.md) — the GUI editor builds on the index-addressed ops landed here and must be revised to pass the chip-strip index (see [GUI follow-up](#gui-follow-up)).

## Goal

Make every sub-profile-scoped `yoke-edit` operation address its target by **0-based positional index** instead of `profile_name`, and carry that change through `yokectl` at full fidelity (arguments, completion, readback, batch `apply`, docs, exit codes). This closes a silent-corruption hole that blocks the GUI editor: real QuadStick profiles do not have unique sub-profile names, so name-keyed ops silently edit the wrong sub-profile.

This slice is self-contained and independently useful from the CLI; it merges before any GUI editor work.

## Why (and why it is critical, not cosmetic)

A QuadStick profile is frequently the user's *only* means of operating the computer. **An edit path that silently mis-targets a sub-profile is a critical bug** — on par with bricking a keyboard for an able-bodied user — because the user may not notice the wrong layer was rewritten until they are mid-game and the controls are gone. The bar is "never silently wrong; refuse when ambiguous."

The shipped design fails that bar. Every sub-profile-scoped op (`AddBinding`, `UpdateBinding`, `ClearBinding`, `SetOverride`, `UnsetOverride`, `DeleteSubProfile`, `RenameSubProfile`, `CloneSubProfile`) names its target by `profile_name`, and `apply` resolves it first-match:

```rust
// yoke-edit/src/apply.rs (current)
fn sub_profile_index(profile, name) -> Result<usize, EditError> {
    profile.sub_profiles.iter()
        .position(|sp| sp.header.profile_name == name)   // FIRST match wins
        .ok_or(SubProfileNotFound { name })
}
```

But the `profile_name` column is **empty for every sub-profile** in real profiles. `examples/default.csv` has **7 sub-profiles, all empty-named** (`Profile Name,,Mouse Mode,` / `Profile Name,,Left joy,` …), disambiguated only by mode/sub-mode/position — and mode is not unique either (three share "Mouse Mode", two share "Left joy"). So a GUI passing the selected sub-profile's (empty) name resolves to **sub-profile 0 every time**: editing layer 2..N silently rewrites layer 1. `RenameSubProfile` cannot even repair this, since it resolves `from` by the same first-match. There is no consumer-side workaround; the engine has no index-addressed path.

**Why it was not caught earlier.** Every existing test addressed a single, *named* sub-profile — the binding-ops proptest literally seeds a sub-profile called `Main`, and the unit tests use inline CSVs with one named layer. The all-empty-name first-match collision was never exercised. The fix below is paired with real-profile fixtures so the regression cannot recur (see [Fixtures and tests](#5-fixtures-and-tests)).

The CLI mostly dodges the corruption today (passing any non-empty name yields a safe `SubProfileNotFound` refusal; only the empty string collides), but it is equally unable to *reach* layers 2..N — so the name model is dead for real profiles in both consumers.

## Addressing model

A sub-profile is addressed by its **0-based position** in `Profile.sub_profiles`, matching CSV row order and the GUI chip strip. `sub_profile`/`index` is a `usize`; an out-of-range value **errors** (`SubProfileIndexOutOfRange`) rather than wrapping or clamping.

**Replay safety.** The GUI routes edits through an op-log (`apply(base, &ops)` re-derived on every change; see the editor spec). Positional indices are safe under that model: each op is applied to the deterministic result of the ops before it, so an op's index always resolves against the same reconstructed state it was recorded against. Example: `[DeleteSubProfile{index:0}, UpdateBinding{sub_profile:1, …}]` — on every replay the delete runs first, so `index:1` in the update consistently denotes the same layer the user saw when they recorded it. Indices do not "drift" because replay reproduces the intermediate states exactly. (Names had the same recorded-against-action-time property; indices keep it while being unambiguous.)

**Names become display-only.** Because identity moves to position, `profile_name` is no longer an identity key. The name-uniqueness invariant (`require_unique_sub_profile_name`) is **dropped**: real profiles legitimately carry empty and duplicate names, so `add`/`clone` must not reject them and `rename` must not enforce uniqueness. `RenameSubProfile` simply sets the display label at an index.

## Changes

### 1. `yoke-edit` — op shape (`op.rs`)

Every sub-profile-scoped variant takes a `usize`; profile-level ops (`SetTitle`, `SetPreference`, `UnsetPreference`) are unchanged. The `#[serde(tag = "op", rename_all = "kebab-case")]` tagging is unchanged, so batch `apply` JSON keeps the same op names with numeric fields.

```rust
AddBinding    { sub_profile: usize, input: String, output: String, modifier: Option<String> }
UpdateBinding { sub_profile: usize, input: String, output: String, modifier: String }
ClearBinding  { sub_profile: usize, input: String, modifier: Option<String> }
SetOverride   { sub_profile: usize, key: String, value: PreferenceValue }
UnsetOverride { sub_profile: usize, key: String }
DeleteSubProfile { index: usize }
RenameSubProfile { index: usize, to: String }
CloneSubProfile  { index: usize, to: String }
AddSubProfile    { name: String, mode: SubProfileMode, sub_mode: String, channel: Channel } // unchanged; appends at end
```

Field naming: scoped ops keep `sub_profile` (now a number — "which sub-profile this edit applies to"); the three management ops use `index` (the sub-profile to delete/rename/clone). `AddSubProfile` keeps `name` (the new layer's display label, which may be empty or duplicate).

### 2. `yoke-edit` — resolution (`apply.rs`)

- Replace `sub_profile_index(profile, name)` with `sub_profile_at(profile, index) -> Result<usize, EditError>`: returns `index` when `index < profile.sub_profiles.len()`, else `SubProfileIndexOutOfRange { index, len }`. Every scoped/management arm calls this instead of the name lookup.
- **Remove `require_unique_sub_profile_name`.** `apply_add_sub_profile` and `apply_clone_sub_profile` push unconditionally; `apply_rename_sub_profile` sets `sub_profiles[index].header.profile_name = to` with no uniqueness check.
- `apply_delete_sub_profile` resolves `index` via `sub_profile_at`, keeps the `LastSubProfileDeletion` guard (refuse when `len == 1`).
- The `(input, modifier)` binding logic inside each sub-profile (add POST / update PUT-with-anchor / clear DELETE-with-optional-modifier) is untouched.

### 3. `yoke-edit` — errors (`error.rs`)

- **Remove** `SubProfileNotFound { name }` and `SubProfileExists { name }` (no name lookup, no name-uniqueness).
- **Add** `SubProfileIndexOutOfRange { index: usize, len: usize }` — `#[error("sub-profile index {index} is out of range (profile has {len} sub-profiles)")]`.
- The binding-error payloads change their sub-profile field from name to index: `BindingExists { sub_profile: usize, input, modifier, output }`, `BindingNotFound { sub_profile: usize, input }`, `AmbiguousBinding { sub_profile: usize, input, output }`. (`UnknownInput`/`UnknownOutput`/`UnknownModifier`/`InvalidModifierArguments`/`LastSubProfileDeletion` are unchanged.)

### 4. `yokectl` — parity

- **Arguments** (`cli.rs`): the `sub_profile` positional on `add-binding`/`update-binding`/`clear-binding` becomes `usize` (clap-parsed); `subprofile delete`/`rename`/`clone` take an index instead of a name. E.g. `yokectl update-binding destiny 2 lip kb_a --modifier "delay_on 250"` (2 = sub-profile index).
- **Completion** (`completion/catalog.rs`): `SubProfileNameCompleter` → `SubProfileIndexCompleter`, which reads the target profile and completes the numeric index with the sub-profile's display label as the candidate description (`2 — Left joy · Normal`), reusing the `editor.rs::sub_label` display convention.
- **Readback** (`commands/view.rs`): `yokectl bindings` numbers sub-profiles 0-based — `GroupedBinding` gains `sub_profile_index`; JSON always carries it, the human view shows `#<index>` per group. Existing `bindings_json` snapshots updated.
- **Batch `apply`**: ops carry numeric `sub_profile`/`index`; no extra wiring (serde).
- **Error envelopes** (`error.rs`): replace `edit-sub-profile-not-found` / `edit-sub-profile-exists` with `edit-sub-profile-index-out-of-range`; binding envelopes unchanged in name. Exit-code classification updated; help and `docs_artifacts` snapshots regenerated.

### 5. Fixtures and tests

Real profiles are checked in as **always-on, CI-enforced** regression fixtures (a deliberate, scoped choice — a `YOKE_CORPUS_DIR` test is skipped in CI because the corpus is not in the repo, so it would not have caught this class):

- Copy `examples/default.csv` → `crates/yoke-edit/tests/fixtures/default.csv` (7 empty-named sub-profiles + the `lip` chord: `kb_left_gui [normal]` and `kb_left_shift [delay_on 1000]` in one layer) and `examples/destiny.csv` → `crates/yoke-edit/tests/fixtures/destiny.csv` (a second real profile for breadth). These are `include_str!`/`include_bytes!`-loaded by the regression tests, so they run on every build.
- The motivating regression test: load `default.csv`, apply `UpdateBinding { sub_profile: 1, … }`, assert sub-profile 0 is byte-identical to the original and sub-profile 1 changed as intended (the corruption that is being fixed).
- `YOKE_CORPUS_DIR` breadth: a **new** `crates/yoke-edit/tests/corpus_edits.rs` (mirroring `yoke-config/tests/corpus_walk.rs`) iterates every profile in the corpus and asserts the no-cross-sub-profile-corruption invariant for index-addressed edits; `yokectl`'s existing `tests/view_corpus.rs` / `tests/e2e_workflows.rs` are extended to exercise index-addressed edits on the real corpus.

`yoke-config`'s "no checked-in CSVs / inline literals only" convention is **unchanged**; checking real fixtures into `yoke-edit`/`yokectl` for this regression class is the scoped exception (precedent: `yoke-gui` already ships `fixtures/default.csv` for its mock).

### 6. `AGENTS.md`

Clarify the Fixtures section: `yoke-config` stays inline-literal-only; `yoke-edit`/`yokectl` may check in real corpus profiles under `tests/fixtures/` as always-on regression fixtures for corruption-class guards. The standing "public `EditOp` change carries its `yokectl` CLI change in the same slice" rule already covers this slice and stays.

## Out of scope

- No `yoke-gui` changes (the editor is a separate slice; see below).
- No CLI name-resolution sugar (accepting a name that uniquely matches and resolving it to an index): the canonical addressing is the index; a friendlier selector can be a later addendum.
- The `(input, modifier)` binding-identity model and its ops' internal logic are unchanged.

## GUI follow-up

The [GUI editor spec](2026-05-30-yoke-gui-editor-design.md) currently sketches its `EditSession` intents as `add_binding(sub, …)` / `clear_binding(sub, …)` etc. with `sub` as a sub-profile reference. Once this slice lands, that spec must be revised so the intents pass the **chip-strip index** (`usize`) straight through to the ops — which is exactly what makes GUI editing unambiguous. No name lookup, no first-match. That revision is a one-line-per-intent change and a note in the editor spec's binding-model section; it is tracked as a follow-up to this slice, not done here.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `yoke-edit` (regression) | `default.csv`: edit sub-profile 1, assert 0 untouched and 1 changed (the corruption fix); index targets the Nth layer across all scoped ops | host `cargo test`, checked-in fixture (always on) |
| `yoke-edit` (ops) | `SubProfileIndexOutOfRange` for every scoped/management op; `add`/`clone` accept empty + duplicate names; `rename`/`delete` by index; `LastSubProfileDeletion` guard; binding add/update/clear still correct under index addressing | host `cargo test`, inline + fixture |
| `yoke-edit` (corpus) | for every profile in `YOKE_CORPUS_DIR`: each index-addressed edit changes only its target layer | `YOKE_CORPUS_DIR=../examples cargo test -p yoke-edit` (local; skipped in bare CI) |
| `yokectl` | index arg parse (good + out-of-range exit codes); completion lists `index — label`; `bindings` numbering + `sub_profile_index` JSON; batch `apply` with numeric refs | CLI test harness + snapshots |
| `yokectl` fuzz | proptest strategies generate indices (in-range and out-of-range), seeded with **multi-sub-profile** profiles (not a single `Main`) so success and out-of-range branches are exercised | `cargo test -p yokectl --test property` |

All standard gates stay green: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the `wasm32` build of `yoke-config` (unaffected — `KEYWORDS`/`keyword()` stay pure `const`).

## Risks and open questions

- **Index opacity in hand-authored batch files.** A `{"op":"update-binding","sub_profile":2,…}` references a position, which is well-defined for a given input file but less self-describing than a name. Mitigation: `yokectl bindings` numbers layers so the index is discoverable; a name-resolution convenience is a possible later addendum.
- **External reordering.** If a profile's sub-profiles are reordered by another tool between reading an index and applying it, the index points elsewhere. This is inherent to positional addressing and out of scope (the GUI reads and edits one in-memory profile; the CLI resolves against the file it was handed).
- **Fixture drift.** The checked-in `default.csv`/`destiny.csv` are snapshots of the corpus; if the corpus evolves they may diverge. Acceptable — they exist to pin the *structural* class (empty/duplicate names, chords), not to mirror the live corpus, which the `YOKE_CORPUS_DIR` tests still cover.

## References

- Engine: [`yoke-edit`](../../../crates/yoke-edit) (`op.rs`, `apply.rs`, `error.rs`).
- CLI: [`yokectl`](../../../crates/yokectl) (`cli.rs`, `commands/edit.rs`, `commands/view.rs`, `completion/catalog.rs`, `error.rs`).
- Binding-identity model this slice preserves: [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md).
- Corpus evidence: `examples/default.csv` — 7 empty-named sub-profiles, `lip` → two outputs in one layer.
- Existing corpus-test pattern to mirror: `crates/yoke-config/tests/corpus_walk.rs`, `crates/yokectl/tests/view_corpus.rs`.
