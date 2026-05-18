use yoke_config::model::{Profile, SubProfile, SubProfileHeader};

use crate::error::{ApplyError, EditError};
use crate::op::EditOp;

pub fn apply(profile: Profile, ops: &[EditOp]) -> Result<Profile, ApplyError> {
    let mut current = profile;
    for (index, op) in ops.iter().enumerate() {
        current = apply_one(current, op).map_err(|error| ApplyError { index, error })?;
    }
    Ok(current)
}

fn apply_one(mut profile: Profile, op: &EditOp) -> Result<Profile, EditError> {
    match op {
        EditOp::SetTitle { title } => {
            profile.top_line.title.clone_from(title);
            Ok(profile)
        }
        EditOp::AddSubProfile {
            name,
            mode,
            sub_mode,
            channel,
        } => {
            if profile
                .sub_profiles
                .iter()
                .any(|sp| sp.header.profile_name == *name)
            {
                return Err(EditError::SubProfileExists { name: name.clone() });
            }
            profile.sub_profiles.push(SubProfile {
                header: SubProfileHeader {
                    profile_name: name.clone(),
                    mode: mode.clone(),
                    sub_mode: sub_mode.clone(),
                    channel: *channel,
                    column_header_label: String::new(),
                },
                rows: vec![],
            });
            Ok(profile)
        }
        EditOp::DeleteSubProfile { name } => {
            let pos = sub_profile_index(&profile, name)?;
            if profile.sub_profiles.len() == 1 {
                return Err(EditError::LastSubProfileDeletion);
            }
            profile.sub_profiles.remove(pos);
            Ok(profile)
        }
        EditOp::RenameSubProfile { from, to } => {
            if profile
                .sub_profiles
                .iter()
                .any(|sp| sp.header.profile_name == *to)
            {
                return Err(EditError::SubProfileExists { name: to.clone() });
            }
            let pos = sub_profile_index(&profile, from)?;
            profile.sub_profiles[pos].header.profile_name.clone_from(to);
            Ok(profile)
        }
        EditOp::CloneSubProfile { from, to } => {
            if profile
                .sub_profiles
                .iter()
                .any(|sp| sp.header.profile_name == *to)
            {
                return Err(EditError::SubProfileExists { name: to.clone() });
            }
            let pos = sub_profile_index(&profile, from)?;
            let mut cloned = profile.sub_profiles[pos].clone();
            cloned.header.profile_name.clone_from(to);
            profile.sub_profiles.push(cloned);
            Ok(profile)
        }
        _ => unimplemented!("op {op:?} not yet supported; coming in later tasks"),
    }
}

fn sub_profile_index(profile: &Profile, name: &str) -> Result<usize, EditError> {
    profile
        .sub_profiles
        .iter()
        .position(|sp| sp.header.profile_name == name)
        .ok_or_else(|| EditError::SubProfileNotFound {
            name: name.to_owned(),
        })
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

    use yoke_config::catalog::{Channel, SubProfileMode};
    use yoke_config::model::{SubProfile, SubProfileHeader};

    fn empty_sp(name: &str) -> SubProfile {
        SubProfile {
            header: SubProfileHeader {
                profile_name: name.into(),
                mode: SubProfileMode::Mouse,
                sub_mode: String::new(),
                channel: Channel::Usb,
                column_header_label: String::new(),
            },
            rows: vec![],
        }
    }

    #[test]
    fn add_sub_profile_appends() {
        let p = empty_profile();
        let out = apply(
            p,
            &[EditOp::AddSubProfile {
                name: "Cougar".into(),
                mode: SubProfileMode::Mouse,
                sub_mode: String::new(),
                channel: Channel::Usb,
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles.len(), 1);
        assert_eq!(out.sub_profiles[0].header.profile_name, "Cougar");
    }

    #[test]
    fn add_sub_profile_rejects_duplicate() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let err = apply(
            p,
            &[EditOp::AddSubProfile {
                name: "Main".into(),
                mode: SubProfileMode::Mouse,
                sub_mode: String::new(),
                channel: Channel::Usb,
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::SubProfileExists {
                name: "Main".into()
            }
        );
    }

    #[test]
    fn delete_sub_profile_removes() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        p.sub_profiles.push(empty_sp("Alt"));
        let out = apply(p, &[EditOp::DeleteSubProfile { name: "Alt".into() }]).unwrap();
        assert_eq!(out.sub_profiles.len(), 1);
        assert_eq!(out.sub_profiles[0].header.profile_name, "Main");
    }

    #[test]
    fn delete_sub_profile_rejects_missing() {
        let p = empty_profile();
        let err = apply(
            p,
            &[EditOp::DeleteSubProfile {
                name: "Ghost".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::SubProfileNotFound {
                name: "Ghost".into()
            }
        );
    }

    #[test]
    fn delete_sub_profile_refuses_last_remaining() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("OnlyOne"));
        let err = apply(
            p,
            &[EditOp::DeleteSubProfile {
                name: "OnlyOne".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(err.error, EditError::LastSubProfileDeletion);
    }

    #[test]
    fn rename_sub_profile_changes_header_name() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let out = apply(
            p,
            &[EditOp::RenameSubProfile {
                from: "Main".into(),
                to: "Cougar".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].header.profile_name, "Cougar");
    }

    #[test]
    fn rename_sub_profile_rejects_target_collision() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        p.sub_profiles.push(empty_sp("Alt"));
        let err = apply(
            p,
            &[EditOp::RenameSubProfile {
                from: "Main".into(),
                to: "Alt".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::SubProfileExists { name: "Alt".into() }
        );
    }

    #[test]
    fn clone_sub_profile_duplicates_rows() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let out = apply(
            p,
            &[EditOp::CloneSubProfile {
                from: "Main".into(),
                to: "MainCopy".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles.len(), 2);
        assert_eq!(out.sub_profiles[1].header.profile_name, "MainCopy");
    }

    #[test]
    fn progressive_validation_add_then_rename() {
        let p = empty_profile();
        let out = apply(
            p,
            &[
                EditOp::AddSubProfile {
                    name: "A".into(),
                    mode: SubProfileMode::Mouse,
                    sub_mode: String::new(),
                    channel: Channel::Usb,
                },
                EditOp::RenameSubProfile {
                    from: "A".into(),
                    to: "B".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].header.profile_name, "B");
    }

    #[test]
    fn batch_failure_returns_index_of_failing_op() {
        let p = empty_profile();
        let err = apply(
            p,
            &[
                EditOp::SetTitle { title: "Ok".into() },
                EditOp::DeleteSubProfile {
                    name: "Ghost".into(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(err.index, 1);
    }
}
