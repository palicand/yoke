use yoke_config::model::Profile;

use crate::error::{ApplyError, EditError};
use crate::op::EditOp;

pub fn apply(profile: Profile, ops: &[EditOp]) -> Result<Profile, ApplyError> {
    let mut current = profile;
    for (index, op) in ops.iter().enumerate() {
        current = apply_one(current, op).map_err(|error| ApplyError { index, error })?;
    }
    Ok(current)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "later tasks add fallible op arms; signature is forward-compatible"
)]
fn apply_one(mut profile: Profile, op: &EditOp) -> Result<Profile, EditError> {
    match op {
        EditOp::SetTitle { title } => {
            profile.top_line.title.clone_from(title);
            Ok(profile)
        }
        _ => unimplemented!("op {op:?} not yet supported; coming in later tasks"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_config::model::{Profile, TopLine};

    fn empty_profile() -> Profile {
        Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
                width: 4,
            },
            sub_profiles: vec![],
            preferences: None,
            infrared: vec![],
        }
    }

    #[test]
    fn set_title_updates_top_line() {
        let p = empty_profile();
        let out = apply(
            p,
            &[EditOp::SetTitle {
                title: "Cougar".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.top_line.title, "Cougar");
    }

    #[test]
    fn empty_ops_returns_input_unchanged() {
        let p = empty_profile();
        let original = p.clone();
        let out = apply(p, &[]).unwrap();
        assert_eq!(out, original);
    }
}
