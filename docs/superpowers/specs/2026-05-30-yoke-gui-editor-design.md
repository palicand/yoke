# Yoke GUI — interactive editor (egui)

**Date:** 2026-05-30
**Revised:** 2026-05-31 — reconciled to the shipped `(input, modifier)` binding model; the original draft assumed an input-keyed `set_binding`/`set_modifier` surface that was discarded in [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md).
**Revised:** 2026-06-07 — reconciled to index-addressed sub-profile ops and the two-argument `yoke_config::write`.
**Stage:** F
**Status:** proposed
**Predecessors:** [`2026-05-27-yoke-gui-egui-design.md`](2026-05-27-yoke-gui-egui-design.md), [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md), [`2026-05-31-yoke-edit-subprofile-addressing-design.md`](2026-05-31-yoke-edit-subprofile-addressing-design.md), [`2026-05-16-yoke-config-design.md`](2026-05-16-yoke-config-design.md), [`2026-05-17-yoke-volume-design.md`](2026-05-17-yoke-volume-design.md), [`2026-05-18-yokectl-design.md`](2026-05-18-yokectl-design.md)

## Goal

Turn the Stage E read-only viewer into an **interactive editor**. The user can change a binding's output or modifier, add a binding to any physical input on a station (including currently-unbound ones and additional parallel chords on an already-bound input), remove a binding, manage sub-profiles (add / clone / rename / delete), undo and redo, preview the resulting CSV, and save back in place, to a new file, or to the mounted QuadStick. Editing runs on both the native build and the `trunk serve` mock build, keeping the browser a full interactive substrate for agents.

## Scope

### In

- **Per-binding editing** via a **modal action picker**, catalog-driven, with a key-capture banner: change an existing binding's output (output mode) or its modifier (modifier mode).
- **Add a binding**: bind a currently-unbound input, or add a parallel chord to an already-bound input, via the same picker.
- **Clear a binding**: remove one specific `(input, modifier)` row, or every row for an input.
- **Full input roster** per selected station: every physical input shows its 0..N binding rows (`modifier → output`); unbound inputs show `(unbound)`.
- **Sub-profile management**: add, clone, rename, delete from the chip strip.
- **Op-log undo/redo** with a dirty indicator and confirm-on-discard.
- **Save** (in place to the open source), **Save As…**, **Save to QuadStick**, and **Preview CSV** before commit.

### Cut from this stage

| Cut | Lands in |
|---|---|
| Preferences / per-sub-profile override **editing** in the GUI (no prefs view exists yet) | later GUI stage |
| Action-picker variant switching (popover / palette) + the Tweaks panel | later GUI stage |
| Diagram / Grid sketch variants, Studio / Contrast themes | later |
| Live device push (writes still go to the mounted FAT volume) | I |
| Windows / Linux | H |
| A user-facing About/credits surface | later |

## Architecture

All editing goes through [`yoke-edit`](../../../crates/yoke-edit): the GUI builds an `EditOp` for each user action and calls `yoke_edit::apply`; it never mutates the `Profile` directly. Routing every edit through the validated engine is what makes the op-log undo/redo below re-derive deterministically and keeps the GUI free of edit-validation logic.

### The binding model this editor consumes

The [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md) slice already shipped the binding-edit vocabulary; this editor is a pure consumer of it. The model the UI must honour:

- A binding row is **`(input, modifier) → output`**. The **`(input, modifier)` pair is the key** and maps to exactly one output. **Input alone is not a key** — one input may legitimately drive several outputs under different modifiers (a chord; corpus proof: `lip` → `kb_left_gui [normal]` *and* `kb_left_shift [delay_on 1000]` in one sub-profile). The "one output per input" mental model is wrong and is precisely the corruption class the engine refuses.
- All sub-profile-scoped ops carry `sub_profile: usize` — the **0-based chip-strip index**, matching CSV row order. This closes the silent-corruption hole where name-keyed ops resolved to the first-match in profiles whose sub-profiles are all empty-named (every real `default.csv` layer). See [`2026-05-31-yoke-edit-subprofile-addressing-design.md`](2026-05-31-yoke-edit-subprofile-addressing-design.md) for the full rationale. **Sub-profile names are display-only**; empty and duplicate names are legal, so `AddSubProfile`/`CloneSubProfile` append unconditionally and `RenameSubProfile` sets the label without enforcing uniqueness.
- The consumer ops (all `EditOp` variants, applied via `yoke_edit::apply`):
  - **`AddBinding { sub_profile: usize, input, output, modifier: Option<String> }`** (POST). `modifier` defaults to `normal`. Errors `BindingExists` if `(input, modifier)` already maps to an output.
  - **`UpdateBinding { sub_profile: usize, input, output, modifier: String }`** (PUT). The engine **anchors** on whichever of `(input, modifier)` / `(input, output)` already matches a single row and changes the other field: matching `(input, modifier)` → set that row's **output**; matching `(input, output)` → set that row's **modifier**; an exact-triple match is a noop. `BindingNotFound` if neither matches; `AmbiguousBinding` if the `(input, output)` anchor matches more than one row.
  - **`ClearBinding { sub_profile: usize, input, modifier: Option<String> }`** (DELETE). `None` removes **every** row for the input; `Some(m)` removes only the unique `(input, m)` row. `BindingNotFound` if nothing matched.
  - Sub-profile management: `AddSubProfile { name, mode, sub_mode, channel }` (appends at end; `name` may be empty or duplicate), `CloneSubProfile { index: usize, to: String }`, `RenameSubProfile { index: usize, to: String }`, `DeleteSubProfile { index: usize }`.
- Engine errors the GUI must reckon with: `BindingExists`, `BindingNotFound`, `AmbiguousBinding`, `UnknownModifier { suggestions }`, `InvalidModifierArguments { keyword, modifier }`, plus the existing `UnknownInput` / `UnknownOutput` / `SubProfileIndexOutOfRange` / `LastSubProfileDeletion`. See [Error handling](#error-handling).

**Criticality.** A profile is frequently the user's only input path; a silent mis-target is a critical bug, not a cosmetic one. The engine's discipline is "never silently wrong; refuse when ambiguous." The GUI inherits that: it addresses a concrete row by its current `(input, modifier)` (always unique), constructs ops that target exactly that row, and **pre-checks `current()` before issuing an op that the engine cannot itself guard** (see modifier-edit below). When the engine still refuses (`AmbiguousBinding`), the GUI surfaces the refusal as a toast rather than working around it.

### Edit-core: `EditSession` (egui-free, dual-target)

A new **egui-free** module `crates/yoke-gui/src/edit.rs` holds the editing state machine, preserving the Stage E "core imports zero egui" discipline (so a later lift into a `yoke-app` crate stays a module move). Chosen over direct `Profile` mutation with a snapshot stack because the op-log reuses the engine's validation verbatim, stays fully testable without egui, and yields the change-list the CSV preview needs.

Authoritative shape lives in the source; the contract:

- `EditSession { base: Profile, template: RawCsv, ops: Vec<EditOp>, redo: Vec<EditOp>, current: Profile, saved: Profile }`. `ParseResult.raw` (the template) threads from `DataSource` through `EditSession::new(parsed: ParseResult)` so template-fidelity writes work without re-reading the file.
- `current()` returns `&Profile` — views read this exactly as they read `OpenProfile.profile` today.
- Each edit intent builds the `EditOp`, applies it with `apply(current, &[op])`, and on success swaps `current`, pushes the op, and clears `redo`; on `EditError` it returns the error unchanged for the UI to toast and leaves state untouched. The intents mirror the engine ops one-to-one:
  - `add_binding(sub: usize, input, output, modifier: Option<&str>)` → `AddBinding`.
  - `update_binding(sub: usize, input, output, modifier: &str)` → `UpdateBinding`.
  - `clear_binding(sub: usize, input, modifier: Option<&str>)` → `ClearBinding`.
  - `add_sub_profile` / `clone_sub_profile(index, to)` / `rename_sub_profile(index, to)` / `delete_sub_profile(index)`.
- `undo()` pops the last op into `redo` and re-derives `current = apply(base, &ops)`; `redo()` replays.
- `is_dirty()` is a **state comparison** (`current != saved`), not `!ops.is_empty()`. This means undoing back to the last-saved shape reads as clean and undoing past a save point reads as dirty — an op-count approach would misreport both.
- `mark_saved(snapshot: Profile)` sets `saved = snapshot`. The snapshot is the profile state captured at **dispatch time** (see save flow), so edits made while a save is in-flight remain dirty after it completes.

`OpenProfile.profile: Profile` (in [`state.rs`](../../../crates/yoke-gui/src/state.rs)) becomes `OpenProfile.session: EditSession`. `yoke-gui` gains an **always** workspace dependency on `yoke-edit` (it is pure — `serde` + `strsim` + `thiserror` + `yoke-config` — so it compiles for `wasm32`).

Re-deriving from `base` on every edit is intentional and cheap — profiles are small — and it makes name-referencing ops robust: rename-then-edit replays correctly because each op was recorded against the state at action time.

### Bindings panel — full input roster, chord-aware (`views/bindings.rs`)

With a station selected, the panel lists **every** physical input belonging to it. `Input` has no single enumerator (it is composed of per-sub-enum `ALL` lists); enumerate via [`Input::all_csv_names()`](../../../crates/yoke-config/src/catalog/inputs.rs) resolved through `Input::from_csv`, then filter by the existing [`input_belongs_to`](../../../crates/yoke-gui/src/stations.rs). Each input groups its **0..N binding rows** (the chord set), read from `current()`:

```text
Station: Lip
────────────────────────────
 lip
   [normal]        -> kb_left_gui    [edit-output] [edit-modifier] [x]
   [delay_on 1000] -> kb_left_shift  [edit-output] [edit-modifier] [x]
   + add binding
 lip_soft        (unbound)           [set]
 side_left       (unbound)           [set]
```

Row / input interactions and the op each builds:

- **Existing binding row** — the GUI knows the row's current `(input, modifier, output)` triple:
  - **Edit output** → picker in **output mode** → `update_binding(sub, input, new_output, modifier_current)`. The engine anchors on `(input, modifier_current)` (unique by the add invariant) and changes the output in place, preserving row order. It refuses with `AmbiguousBinding` only in the symmetric corner where `new_output` already maps from this same input under a *different* modifier — both anchors then resolve to different rows and the engine cannot tell an output-change from a modifier-change.
  - **Edit modifier** → picker in **modifier mode** → `update_binding(sub, input, output_current, new_modifier)`. The engine anchors on `(input, output_current)` and changes the modifier. See the pre-check and ambiguity note below.
  - **Clear (`x`)** → `clear_binding(sub, input, Some(modifier_current))` — removes exactly that row.
- **`+ add binding`** under a bound input → picker in **output mode**; commits `add_binding(sub, input, output, modifier)` to create a parallel chord. The picker's modifier sub-control supplies the chord's modifier (default `normal`).
- **Unbound input `[set]`** → picker in **output mode** → `add_binding(sub, input, output, modifier)`.
- **Clear all for an input** (input-level affordance) → `clear_binding(sub, input, None)`.

This makes add and edit **distinct** (POST vs PUT), matching the engine: a `(unbound)` input or a new chord is an `add`; mutating an existing row is an `update`. (The original draft's claim that the roster "unifies edit and add — no separate add flow" was predicated on the discarded input-keyed model and no longer holds.)

With **no** station selected, the panel keeps the Stage E read-only "all existing bindings" browse — the all-inputs-across-all-stations roster is too long to be a useful editing surface.

#### Modifier-edit anchoring: pre-check and ambiguity

`UpdateBinding` resolves the target by matching either anchor; `(input, modifier)` is unique by the add invariant but `(input, output)` is **not** (the same input may map to the same output under two different modifiers, e.g. `lip → kb_a [normal]` and `lip → kb_a [toggle]`). So `AmbiguousBinding` is reachable from **both** edit directions: the modifier path when `(input, output_current)` keys several rows, and the output path in the symmetric corner above. Two consequences the GUI must handle, both rooted in the corruption-safety bar:

1. **Ambiguous anchor.** When the engine cannot resolve to a single row it returns `AmbiguousBinding` rather than guessing, and the GUI surfaces it as a toast. This is the safe outcome; the precise-but-order-changing alternative — address the row by its unique `(input, old_modifier)` via `clear_binding(input, Some(old_modifier))` + `add_binding(input, new_output, new_modifier)` — is deferred because it reorders rows and so perturbs CSV template fidelity. Listed as an open question.
2. **Duplicate-key creation.** Setting a row's modifier to one that already keys another row for the same input would create a duplicate `(input, modifier)` — which the engine's modifier path does **not** itself reject. The GUI therefore **pre-checks `current()`** before issuing the op: if `(input, new_modifier)` already exists for the input, it refuses inline (the modifier sub-control disables the colliding keyword / shows a toast) instead of producing a silently duplicated key.

### Action picker — modal, catalog-driven (`views/picker.rs`)

A single variant: `egui::Modal` (egui 0.34 native). Picker-variant switching (popover / palette) is deferred with the Tweaks panel. The picker always operates on a **concrete target**: either an existing row (`update`) or an input awaiting a new binding (`add`).

- **Output mode** (used by add, edit-output, and add-chord):
  - **Categories come from the `Output` enum variant** — `Keyboard / Mouse / Gamepad / Dpad / Joystick / System / Touch` are exactly the design-handoff category set — so there is **no parallel presentation table** to author or keep in sync. The authoritative list is [`Output::iter_known()`](../../../crates/yoke-config/src/catalog/outputs.rs).
  - Search box filters by csv id / derived label; category chips filter by variant.
  - On add, the picker also exposes the modifier sub-control (below), defaulting to `normal`.
  - Selecting an entry commits `add_binding` (add / chord) or `update_binding` anchored by the row's current modifier (edit-output).
  - **Key-capture banner** (output mode only): arm it, map the next `egui::Event::Key` to a `kb_*` output (a native port of the handoff's `eventToOutputId` in [`picker.jsx`](../../../../design_handoff_quadstick_config/src/picker.jsx)), and commit directly. Unmappable keys show an inline message. **Coverage notes:** (1) egui's `Key` enum has no modifier-key variants (Shift, Ctrl, Alt, GUI live in `Modifiers`, not `Key`), so all sided `kb_left_*` / `kb_right_*` modifier outputs remain list-only — they cannot be capture-mapped. (2) Several `kb_*` outputs whose ids were present in the plan-era handoff are absent from the `Output` catalog and therefore also cannot be capture-mapped: `kb_home`, `kb_end`, `kb_page_up`, `kb_page_down`, `kb_insert`, and most punctuation keys (only `kb_slash` is catalog-present and capture-mapped). For unmapped egui keys or catalog-absent ids, the banner shows an inline message and falls back to the searchable list.
- **Modifier mode** (used by edit-modifier): see below.

### Modifier editing — type + arguments (modifier mode)

Modifier mode lists the modifier keywords from [`Modifier::KEYWORDS`](../../../crates/yoke-config/src/catalog/modifiers.rs) — the 14-entry catalog primitive shipped alongside the ops — rather than re-enumerating the `Modifier` variants ad hoc. Selecting a keyword that carries arguments (`delay_on {ms}`, `greater_than {pct, upper}`, `repeat {hz, delay_ms}`, `pulse {ms, count}`, …) reveals inline numeric fields, validated against the variant's argument shape via `Modifier::from_csv` round-tripping. The committed value is the modifier's csv form, applied through `update_binding` (anchored by the row's current output) or, when adding, carried on the `add_binding` op. Selecting `normal` / `toggle` (no arguments) clears the fields.

Argument validation happens **inline before commit**: a field that would yield `InvalidModifierArguments` (bad/extra/missing argument for a recognised keyword) blocks the commit button with an inline message, so the engine error is normally pre-empted. The keyword set is closed (driven by `KEYWORDS`), so `UnknownModifier` is unreachable from the GUI.

**Unrecognized-modifier refusal.** When a binding row's existing modifier came from a hand-edited CSV with an unknown keyword or more argument tokens than the catalog schema allows, editing would silently truncate the extra tokens on Apply. Rather than corrupt the profile, modifier mode detects this condition via `is_unrecognized_modifier` (unknown keyword, or `actual_args > expected_args`) and **refuses to open the modifier editor**, showing the raw modifier value read-only with a "Unrecognized modifier — edit refused to avoid data loss" message and only a Cancel button. The output-edit and clear affordances on that row remain available. The picker stays open until the user explicitly cancels (no backdrop-dismiss-to-commit path).

### Sub-profile management (`views/editor.rs` strip)

The existing chip strip gains affordances mapping to `yoke-edit`'s sub-profile operations:

- **Add** — small inline form: name (may be empty or duplicate) + [`SubProfileMode`](../../../crates/yoke-config/src/catalog/subprofile_modes.rs) + sub-mode + [`Channel`](../../../crates/yoke-config/src/catalog/channels.rs). Appends at the end; `AddSubProfile` never rejects an empty or duplicate name.
- **Clone** (`CloneSubProfile { index, to }`) — clones the chip-strip–selected layer.
- **Rename** (`RenameSubProfile { index, to }`) — inline edit; name-uniqueness is **not** enforced (display-only label).
- **Delete** (`DeleteSubProfile { index }`) — the engine refuses deleting the last remaining sub-profile via `LastSubProfileDeletion`; surface that as a toast. There is no `SubProfileExists` error (name collisions are legal).

### Save flow (`views/editor.rs` header + worker)

Header toolbar gains a dirty marker and three actions, plus a preview:

- **Save** — write in place to the open source. `File` → overwrite the file; `Device` → write to the mounted volume; `Community` (remote) → disabled (no in-place target), fall through to Save As / Save to QuadStick.
- **Save As…** — native file picker (`rfd`), write a new CSV. **Native only.**
- **Save to QuadStick** — write to the mounted volume regardless of source. **Native only.**
- **Preview CSV** — modal showing `String::from_utf8_lossy(&session.serialize()?)` in a monospace scroll area (uses the same `serialize` path as actual saves, including template-fidelity and canonical fallback), with a Save action that delegates to `save_in_place`. The Save button is disabled for `Community` sources and when a save is in-flight.

Serialization is `EditSession::serialize()`, which calls `yoke_config::write(&profile, Some(&template))` for template-fidelity (preserves row order and whitespace of the original CSV). When a structural sub-profile edit (add/clone/delete) changes the section count the template was laid out for, `write` returns `WriteError::InvariantViolation`; `serialize` catches this and falls back to `yoke_config::write(&profile, None)` (canonical layout), logging a warning. This mirrors `yokectl`'s `load_apply_save` fallback.

Writes go through new `DataSource` methods and the worker, never on the UI thread:

- `DataSource::write_file_profile(&Path, &[u8])` and `write_device_profile(&ProfileName, &[u8])` (the latter delegates to [`VolumeProvider::write_profile`](../../../crates/yoke-volume/src/provider.rs)).
- Commands `AppCommand::{SaveFile { req, path, bytes }, SaveDevice { req, name, bytes }, SaveAsDialog { req, bytes }}` carry **pre-serialized bytes** (serialization is pure and cheap, done on the UI thread before dispatch). `SaveAsDialog` is routed through the worker's native file-dialog site like `OpenFileDialog`; on wasm it falls through to a benign cancellation.
- `DataEvent::Saved { req, label }` clears the dirty state and toasts success; `FailureContext::{SaveFile, SaveDevice}` carry write failures to a toast. Save uses the same monotonic `req` staleness reconciliation as opens.
- **One save in flight at a time.** `dispatch_save` refuses a second dispatch when `pending_save` is already set (toasts "A save is already in progress" and returns early). This prevents two concurrent writes to the same target from landing out of order and leaving stale bytes on disk while the UI reads clean.
- **Snapshot semantics.** `dispatch_save` captures `session.current().clone()` at dispatch time and stores it in `pending_save: Option<(u64, Profile)>`. When `DataEvent::Saved` arrives, `mark_saved` receives this dispatch-time snapshot — not the current state — so edits made while the save was in-flight remain dirty.
- **Save-to-QuadStick name derivation.** `save_to_device` derives the `ProfileName` from: the device profile's filename (`name.as_filename()`) when the source is a `Device`; the file stem when the source is a `File`; the community entry's display name when the source is `Community`. The raw string is validated by `ProfileName::new`; invalid names surface as a toast without dispatching.

`MockDataSource` implements the save methods as an in-memory/no-op success so the wasm dev build and host tests exercise the full flow.

### State, threading, wasm

- `app.rs` holds picker state (open / mode / target — an existing row's `(input, modifier, output)` for update, or an input for add), and a confirm-on-discard guard: Back / Escape / opening another profile while `session.is_dirty()` prompts before discarding. Escape's existing back-stepping (station → profile → pending open) gains the dirty prompt at the profile-close step.
- Editing, undo/redo, the picker, and the CSV preview run on **both** targets. Only the three native save paths (`SaveAsDialog`, `save_to_device`, and the on-disk `SaveFile`/`SaveDevice` for `File`/`Device` sources) are `cfg`-gated off wasm, mirroring how the file-open button is already gated.
- Egui-free-core rule preserved: `edit.rs`, `data/`, `state.rs`, `stations.rs`, and the command/event protocol import zero egui; only `app.rs` and `views/` touch egui.

### Error handling

Engine `EditError`s surface as toasts. The picker offers only valid catalog entries and the modifier editor validates arguments inline, so the catalog-validity errors (`UnknownInput` / `UnknownOutput` / `UnknownModifier` / `InvalidModifierArguments`) are normally unreachable from the GUI; they remain wired as a backstop. The reachable ones, and how the editor treats them:

| Error | When it can occur from the GUI | Treatment |
|---|---|---|
| `BindingExists` | `add` / add-chord whose `(input, modifier)` already exists | toast; the add picker pre-checks `current()` and disables the colliding modifier where feasible |
| `AmbiguousBinding` | edit-modifier when `(input, output)` keys more than one row, or edit-output when the new output already maps from the same input under another modifier | toast (the safe refusal); precise clear+add fallback deferred |
| `BindingNotFound` | clear / update against a row that vanished (stale UI) | toast; the roster re-reads `current()` after every op so this is rare |
| `LastSubProfileDeletion` | delete the only remaining sub-profile | toast |
| `SubProfileIndexOutOfRange` | sub-profile index >= len; backstop only — the GUI clamps `selected_subprofile` to valid range and stamps it at picker-open time, so this is unreachable in practice | toast |

Save errors surface as toasts and leave the in-memory session dirty (not lost). The Stage E disconnect-mid-session behavior is unchanged: a write that fails because the volume vanished toasts and the editor stays open on the in-memory profile.

## On-disk layout

```text
crates/yoke-gui/src/
  edit.rs               # NEW: EditSession (egui-free, dual-target); add/update/clear_binding + sub-profile intents
  state.rs              # OpenProfile.profile -> OpenProfile.session: EditSession
  data/
    mod.rs              # + AppCommand::{SaveFile,SaveDevice,SaveAsDialog} (pre-serialized bytes), DataEvent::Saved{req,label}, FailureContext::{SaveFile,SaveDevice}, DataSource save methods
    native.rs           # + write_file_profile / write_device_profile
    mock.rs             # + in-memory save success
  worker.rs             # + route Save commands; SaveAs via the native dialog site
  app.rs                # + picker state (row/input target), edit dispatch, dirty/confirm-on-discard, save actions
  views/
    picker.rs           # NEW: modal action picker (output + modifier modes, key-capture, modifier sub-control)
    bindings.rs         # full input roster, chord-aware: per-row edit-output/edit-modifier/clear, per-input add/clear-all
    editor.rs           # save toolbar + dirty marker + sub-profile management + CSV preview modal
```

## Dependencies

Added via `cargo add` per [`AGENTS.md`](../../../AGENTS.md). `yoke-gui` gains an **always** workspace dependency on `yoke-edit` (dual-target). No new third-party crates: the modal is `egui::Modal` (0.34), the file dialog reuses `rfd`, serialization reuses `yoke_config::write`, and the volume write path reuses `yoke-volume`.

## Tooling and CI

No new gates. The existing Stage E gates continue to apply and must stay green: `cargo build -p yoke-gui` (native), `cargo test -p yoke-gui`, `cargo clippy --workspace --all-targets -- -D warnings`, and `trunk build` inside `crates/yoke-gui` (wasm). The wasm build gate enforces that the edit-core stays dual-target.

## Testing

| Layer | Coverage | Mechanism |
|---|---|---|
| `EditSession` | add / update (output-anchored and modifier-anchored) / clear (scoped and all); dirty flag (state comparison, not op count: undo-to-saved reads clean, undo-past-save reads dirty); `mark_saved` clears dirty and undo past the save point restores dirty; re-derivation after rename-then-edit; error leaves state untouched | host `cargo test` (egui-free) |
| Chord handling | an input with multiple `(input, modifier)` rows enumerates all of them; clear-one vs clear-all; add-chord vs `BindingExists` | host `cargo test` |
| Index isolation | add/update/clear on sub-profile 1 leaves sub-profile 0 byte-identical (the corruption class the index slice fixed) | host `cargo test` |
| Modifier-edit safety | edit-modifier pre-check refuses a duplicate `(input, modifier)`; `AmbiguousBinding` surfaces (not silently resolved) when `(input, output)` is non-unique; `is_unrecognized_modifier` refuses unknown keywords and extra-token modifiers | host `cargo test` |
| Picker mapping | `egui::Event::Key` → `kb_*` for letters/digits/named keys; sided modifier keys (`kb_left_shift` etc.) are not reachable via egui Key (list-only); catalog-absent keys (`kb_home` etc.) return `None`; unmappable keys rejected; every mapped id exists in the catalog | host `cargo test` (egui-free mapping + validation fns) |
| Roster | per-station input enumeration via `input_belongs_to`; unbound inputs rendered as `(unbound)` | host `cargo test` |
| Save | mock save round-trip; `Saved`/`Failed` event reconciliation by `req`; second dispatch refused while `pending_save` is set; stale `Saved` event dropped | host `cargo test` against `MockDataSource` |

`egui_kittest` visual-regression tests remain deferred (consistent with Stage E). Manual acceptance: native run — open a device profile, change an input's output via the picker, change a binding's modifier to `delay_on` with an argument, add a parallel chord to the same input, add and rename a sub-profile (empty name accepted), undo back to clean, preview the CSV, save to the volume; `trunk serve` — the same edit/undo/redo/picker flow against the mock (save is the in-memory no-op).

## Risks and open questions

- **Edit ambiguity vs CSV fidelity.** `UpdateBinding` preserves row order but refuses (`AmbiguousBinding`) whenever the anchor it must use is non-unique — modifier-edit when `(input, output)` repeats, output-edit when the new output already maps from the same input under another modifier. The precise alternative — `clear_binding(input, Some(old_modifier))` then `add_binding(input, new_output, new_modifier)`, addressing by the always-unique `(input, modifier)` — handles every case but reorders rows, perturbing `yoke_config::write` template fidelity and the save diff. Deferred; revisit if duplicate-output-different-modifier chords prove common. Until then the GUI takes the order-preserving path and surfaces the refusal.
- **Duplicate-key creation on modifier-edit.** The engine's `UpdateBinding` modifier path does not re-check `(input, new_modifier)` uniqueness. The GUI pre-checks `current()` and refuses inline; a future engine guard (or a by-`(input, modifier)` update op) would let the GUI drop the pre-check.
- **Key-capture coverage.** egui's key event set is smaller than the browser's `KeyboardEvent`; some `kb_*` outputs may have no key event (e.g. keypad). The banner degrades gracefully (unmappable → inline message + fall back to the searchable list); full coverage is not required.
- **Save-to-device safety.** Writing to the mounted FAT volume mid-session races with macOS volume enumeration and with the user yanking the device. Reuse the Stage E disconnect-mid-session behavior; a failed write surfaces as a toast and leaves the in-memory session dirty.
- **Community in-place save.** Confirmed out of scope: remote URLs are not writable; Save is disabled for `Community` sources, leaving Save As / Save to QuadStick.

## References

- Stage E read-only viewer spec: [`2026-05-27-yoke-gui-egui-design.md`](2026-05-27-yoke-gui-egui-design.md).
- Binding-edit ops + CLI parity this editor consumes: [`2026-05-30-yoke-edit-binding-ops-design.md`](2026-05-30-yoke-edit-binding-ops-design.md) — the `(input, modifier)` model, `AddBinding`/`UpdateBinding`/`ClearBinding`, and the `BindingExists`/`BindingNotFound`/`AmbiguousBinding`/`UnknownModifier`/`InvalidModifierArguments` error set.
- Index-addressed sub-profile ops (supersedes name-keyed model): [`2026-05-31-yoke-edit-subprofile-addressing-design.md`](2026-05-31-yoke-edit-subprofile-addressing-design.md).
- Action-picker UX (visual reference; not a port target): `../../../../design_handoff_quadstick_config/src/picker.jsx` (modal/popover/palette variants, `eventToOutputId` key mapping) and `data.js` (representative output/category/modifier tables).
- Edit engine consumed by the GUI: [`yoke-edit`](../../../crates/yoke-edit) — `EditOp`, `apply`, `error.rs`.
- Serialization: [`yoke_config::write`](../../../crates/yoke-config/src/csv/write.rs). Volume write: [`yoke-volume`](../../../crates/yoke-volume) `VolumeProvider::write_profile`.
- Catalog enums driving the picker: [`Output`](../../../crates/yoke-config/src/catalog/outputs.rs), [`Modifier`](../../../crates/yoke-config/src/catalog/modifiers.rs) (incl. `Modifier::KEYWORDS`), [`Input`](../../../crates/yoke-config/src/catalog/inputs.rs).
