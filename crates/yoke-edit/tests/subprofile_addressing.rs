//! Index-addressing regression suite. Uses real corpus profiles checked in under
//! `tests/fixtures/` so the multi-empty-name structure that defeated name-based
//! addressing is exercised on every CI build (a `YOKE_CORPUS_DIR` test is skipped in
//! bare CI and would not have caught this).

use yoke_config::catalog::Input;
use yoke_config::model::Profile;
use yoke_edit::{EditError, EditOp, apply};

const DEFAULT_CSV: &[u8] = include_bytes!("fixtures/default.csv");

fn parse(bytes: &[u8]) -> Profile {
    yoke_config::parse(bytes).expect("fixture parses").model
}

#[test]
fn default_fixture_has_multiple_empty_named_sub_profiles() {
    // The premise of this whole slice: real profiles do not have unique sub-profile
    // names, so name-first-match silently targets the wrong layer.
    let p = parse(DEFAULT_CSV);
    assert!(
        p.sub_profiles.len() >= 2,
        "need >=2 sub-profiles to demonstrate the collision, got {}",
        p.sub_profiles.len()
    );
    let empty = p
        .sub_profiles
        .iter()
        .filter(|sp| sp.header.profile_name.is_empty())
        .count();
    assert!(
        empty >= 2,
        "expected >=2 empty-named sub-profiles (the collision case), got {empty}"
    );
}

// Re-serialize one sub-profile to compare layers without depending on PartialEq of
// the whole profile (other layers must stay byte-identical through a single edit).
fn layer_csv(p: &Profile, idx: usize) -> Vec<u8> {
    let mut one = p.clone();
    one.sub_profiles = vec![p.sub_profiles[idx].clone()];
    yoke_config::write(&one, None).expect("canonical write")
}

#[test]
fn update_binding_targets_the_indexed_layer_only() {
    let before = parse(DEFAULT_CSV);
    // Pick an input that exists in layer 1 so the update resolves there. `lip` is
    // bound in every layer of default.csv; layer 1 binds `lip [normal] -> kb_left_gui`.
    let idx = 1usize;
    let untouched_before = layer_csv(&before, 0);

    let after = apply(
        before,
        &[EditOp::UpdateBinding {
            sub_profile: idx,
            input: "lip".into(),
            output: "kb_escape".into(),
            modifier: "normal".into(),
        }],
    )
    .expect("update applies to layer 1");

    // Layer 0 is byte-identical: the edit did NOT bleed into the first layer (the bug).
    assert_eq!(
        layer_csv(&after, 0),
        untouched_before,
        "editing layer 1 must not modify layer 0"
    );
    // The retargeted row specifically: lip[normal] now outputs kb_escape (not just
    // "kb_escape appears somewhere", which is vacuously true in default.csv).
    let changed = after.sub_profiles[idx].bindings().any(|b| {
        b.input.as_ref().map(Input::to_csv).as_deref() == Some("lip")
            && b.modifier.to_csv() == "normal"
            && b.output.to_csv() == "kb_escape"
    });
    assert!(
        changed,
        "the lip[normal] row in layer 1 should now output kb_escape"
    );
    // The update replaced rather than added: the old output is gone from that exact row.
    let old_gone = !after.sub_profiles[idx].bindings().any(|b| {
        b.input.as_ref().map(Input::to_csv).as_deref() == Some("lip")
            && b.modifier.to_csv() == "normal"
            && b.output.to_csv() == "kb_left_gui"
    });
    assert!(
        old_gone,
        "lip[normal] should no longer output kb_left_gui after the update"
    );
}

#[test]
fn out_of_range_index_errors_not_panics() {
    let p = parse(DEFAULT_CSV);
    let len = p.sub_profiles.len();
    let err = apply(
        p,
        &[EditOp::ClearBinding {
            sub_profile: 999,
            input: "lip".into(),
            modifier: None,
        }],
    )
    .expect_err("out-of-range index must error");
    assert_eq!(
        err.error,
        EditError::SubProfileIndexOutOfRange { index: 999, len }
    );
}
