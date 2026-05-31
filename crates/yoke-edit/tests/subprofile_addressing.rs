//! Index-addressing regression suite. Uses real corpus profiles checked in under
//! `tests/fixtures/` so the multi-empty-name structure that defeated name-based
//! addressing is exercised on every CI build (a YOKE_CORPUS_DIR test is skipped in
//! bare CI and would not have caught this).

use yoke_config::model::Profile;

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
