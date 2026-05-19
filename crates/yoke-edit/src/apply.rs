use yoke_config::catalog::{
    DPadDir, GamepadButton, Input, JoyAxis, JoyOutput, KbKey, Modifier, MouseAction, Output,
    PreferenceKey, PreferenceSpec, PreferenceValueKind, SipPuff, SystemAction, UsbHost,
};
use yoke_config::model::{
    Binding, PreferenceEntry, PreferenceOverride, Preferences, Profile, SubProfile,
    SubProfileHeader, SubProfileRow,
};

use crate::error::{ApplyError, EditError};
use crate::op::{EditOp, PreferenceValue};
use crate::suggest::suggestions;

pub fn apply(profile: Profile, ops: &[EditOp]) -> Result<Profile, ApplyError> {
    let mut current = profile;
    for (index, op) in ops.iter().enumerate() {
        current = apply_one(current, op).map_err(|error| ApplyError { index, error })?;
    }
    Ok(current)
}

fn apply_one(profile: Profile, op: &EditOp) -> Result<Profile, EditError> {
    match op {
        EditOp::SetTitle { title } => Ok(apply_set_title(profile, title)),
        EditOp::AddSubProfile {
            name,
            mode,
            sub_mode,
            channel,
        } => apply_add_sub_profile(profile, name, mode, sub_mode, *channel),
        EditOp::DeleteSubProfile { name } => apply_delete_sub_profile(profile, name),
        EditOp::RenameSubProfile { from, to } => apply_rename_sub_profile(profile, from, to),
        EditOp::CloneSubProfile { from, to } => apply_clone_sub_profile(profile, from, to),
        EditOp::SetBinding {
            sub_profile,
            input,
            output,
        } => apply_set_binding(profile, sub_profile, input, output),
        EditOp::ClearBinding { sub_profile, input } => {
            apply_clear_binding(profile, sub_profile, input)
        }
        EditOp::SetPreference { key, value } => apply_set_preference(profile, key, value),
        EditOp::UnsetPreference { key } => Ok(apply_unset_preference(profile, key)),
        EditOp::SetOverride {
            sub_profile,
            key,
            value,
        } => apply_set_override(profile, sub_profile, key, value),
        EditOp::UnsetOverride { sub_profile, key } => {
            apply_unset_override(profile, sub_profile, key)
        }
    }
}

fn apply_set_title(mut profile: Profile, title: &str) -> Profile {
    profile.top_line.title.clear();
    profile.top_line.title.push_str(title);
    profile
}

fn apply_add_sub_profile(
    mut profile: Profile,
    name: &str,
    mode: &yoke_config::catalog::SubProfileMode,
    sub_mode: &str,
    channel: yoke_config::catalog::Channel,
) -> Result<Profile, EditError> {
    require_unique_sub_profile_name(&profile, name)?;
    profile.sub_profiles.push(SubProfile {
        header: SubProfileHeader {
            profile_name: name.to_owned(),
            mode: mode.clone(),
            sub_mode: sub_mode.to_owned(),
            channel,
            column_header_label: String::new(),
        },
        rows: vec![],
    });
    Ok(profile)
}

fn apply_delete_sub_profile(mut profile: Profile, name: &str) -> Result<Profile, EditError> {
    let pos = sub_profile_index(&profile, name)?;
    if profile.sub_profiles.len() == 1 {
        return Err(EditError::LastSubProfileDeletion);
    }
    profile.sub_profiles.remove(pos);
    Ok(profile)
}

fn apply_rename_sub_profile(
    mut profile: Profile,
    from: &str,
    to: &str,
) -> Result<Profile, EditError> {
    require_unique_sub_profile_name(&profile, to)?;
    let pos = sub_profile_index(&profile, from)?;
    to.clone_into(&mut profile.sub_profiles[pos].header.profile_name);
    Ok(profile)
}

fn apply_clone_sub_profile(
    mut profile: Profile,
    from: &str,
    to: &str,
) -> Result<Profile, EditError> {
    require_unique_sub_profile_name(&profile, to)?;
    let pos = sub_profile_index(&profile, from)?;
    let mut cloned = profile.sub_profiles[pos].clone();
    to.clone_into(&mut cloned.header.profile_name);
    profile.sub_profiles.push(cloned);
    Ok(profile)
}

fn require_unique_sub_profile_name(profile: &Profile, name: &str) -> Result<(), EditError> {
    if profile
        .sub_profiles
        .iter()
        .any(|sp| sp.header.profile_name == name)
    {
        return Err(EditError::SubProfileExists {
            name: name.to_owned(),
        });
    }
    Ok(())
}

fn apply_set_binding(
    mut profile: Profile,
    sub_profile: &str,
    input: &str,
    output: &str,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let parsed_input = parse_input(input)?;
    let parsed_output = parse_output(output)?;
    let target = &mut profile.sub_profiles[sp_idx];
    let existing = target.rows.iter_mut().find(|r| match r {
        SubProfileRow::Binding(b) => b.input.as_ref() == Some(&parsed_input),
        SubProfileRow::Override(_) => false,
    });
    if let Some(SubProfileRow::Binding(b)) = existing {
        b.output = parsed_output;
    } else {
        target.rows.push(SubProfileRow::Binding(Binding::new(
            parsed_output,
            Modifier::Normal,
            Some(parsed_input),
        )));
    }
    Ok(profile)
}

fn apply_clear_binding(
    mut profile: Profile,
    sub_profile: &str,
    input: &str,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let parsed_input = parse_input(input)?;
    let target = &mut profile.sub_profiles[sp_idx];
    let before = target.rows.len();
    target.rows.retain(|r| match r {
        SubProfileRow::Binding(b) => b.input.as_ref() != Some(&parsed_input),
        SubProfileRow::Override(_) => true,
    });
    if target.rows.len() == before {
        // Catalog-valid identifier with no row in this scope reads as "unknown" to the user.
        return Err(EditError::UnknownInput {
            input: input.to_owned(),
            suggestions: vec![],
        });
    }
    Ok(profile)
}

fn parse_input(raw: &str) -> Result<Input, EditError> {
    match Input::from_csv(raw) {
        Input::Unknown(_) => Err(EditError::UnknownInput {
            input: raw.to_owned(),
            suggestions: suggestions(raw, input_csv_names().iter().map(String::as_str)),
        }),
        ok => Ok(ok),
    }
}

fn parse_output(raw: &str) -> Result<Output, EditError> {
    match Output::from_csv(raw) {
        Output::Unknown(_) => Err(EditError::UnknownOutput {
            output: raw.to_owned(),
            suggestions: suggestions(raw, output_csv_names().iter().map(String::as_str)),
        }),
        ok => Ok(ok),
    }
}

fn input_csv_names() -> Vec<String> {
    use yoke_config::catalog::MpPosition;
    let mut out: Vec<String> = Vec::new();
    for dir in SipPuff::ALL {
        for soft in [false, true] {
            let suffix = if soft { "_soft" } else { "" };
            for pos in MpPosition::ALL {
                out.push(format!("mp_{}_{}{suffix}", pos.as_csv(), dir.as_csv()));
            }
            out.push(format!("right_{}{suffix}", dir.as_csv()));
        }
        out.push(format!("right_{}_long", dir.as_csv()));
    }
    out.push("lip".into());
    out.push("lip_soft".into());
    for ax in JoyAxis::ALL {
        out.push((*ax).as_csv().to_owned());
    }
    out.push("any_direction".into());
    out.push("center".into());
    out.push("constant".into());
    for d in DPadDir::ALL {
        out.push((*d).as_csv().to_owned());
        out.push(format!("{}_inner", d.as_csv()));
    }
    for host in UsbHost::ALL {
        let h = host.as_csv_index();
        for ax in JoyAxis::ALL {
            out.push(format!("usb_{h}_{}", ax.as_csv()));
        }
        for d in DPadDir::ALL {
            out.push(format!("usb_{h}_{}", d.as_csv()));
            out.push(format!("usb_{h}_{}_inner", d.as_csv()));
        }
        for n in 1u8..=15 {
            out.push(format!("usb_{h}_button_{n}"));
        }
    }
    for n in 1u8..=8 {
        out.push(format!("digital_in_{n}"));
    }
    out
}

fn output_csv_names() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for k in KbKey::ALL {
        out.push((*k).as_csv().to_owned());
    }
    for m in MouseAction::ALL {
        out.push((*m).as_csv().to_owned());
    }
    for g in GamepadButton::ALL {
        out.push((*g).as_csv().to_owned());
    }
    for d in DPadDir::ALL {
        out.push(format!("dpad_{}", d.as_csv()));
    }
    for j in JoyOutput::ALL {
        out.push((*j).as_csv().to_owned());
    }
    for s in SystemAction::ALL {
        out.push((*s).as_csv().to_owned());
    }
    out.push("touch".into());
    out
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

fn lookup_preference_spec(key: &str) -> Result<PreferenceSpec, EditError> {
    PreferenceSpec::for_id(key).ok_or_else(|| EditError::UnknownPreference {
        key: key.to_owned(),
        suggestions: suggestions(key, PreferenceSpec::ALL.iter().map(|s| s.id)),
    })
}

fn coerce_value(spec: &PreferenceSpec, value: &PreferenceValue) -> Result<String, EditError> {
    let raw = match (&spec.kind, value) {
        (
            PreferenceValueKind::IntRange { .. } | PreferenceValueKind::SelectInt(_),
            PreferenceValue::Number(n),
        ) => n.to_string(),
        (PreferenceValueKind::Bool, PreferenceValue::Bool(b)) => {
            if *b { "1" } else { "0" }.to_owned()
        }
        (PreferenceValueKind::Bool, PreferenceValue::Number(n)) if *n == 0 || *n == 1 => {
            n.to_string()
        }
        (PreferenceValueKind::Select(_) | PreferenceValueKind::Text, PreferenceValue::Text(s)) => {
            s.clone()
        }
        _ => {
            return Err(EditError::InvalidPreferenceValue {
                key: spec.id.to_owned(),
                value: format!("{value:?}"),
                expected_type: kind_label(&spec.kind).to_owned(),
            });
        }
    };
    spec.validate(&raw)
        .map_err(|msg| EditError::InvalidPreferenceValue {
            key: spec.id.to_owned(),
            value: raw.clone(),
            expected_type: msg,
        })?;
    Ok(raw)
}

const fn kind_label(kind: &PreferenceValueKind) -> &'static str {
    match kind {
        PreferenceValueKind::IntRange { .. } | PreferenceValueKind::SelectInt(_) => "integer",
        PreferenceValueKind::Bool => "boolean",
        PreferenceValueKind::Select(_) => "string",
        PreferenceValueKind::Text => "text",
    }
}

fn apply_set_preference(
    mut profile: Profile,
    key: &str,
    value: &PreferenceValue,
) -> Result<Profile, EditError> {
    let spec = lookup_preference_spec(key)?;
    let raw = coerce_value(&spec, value)?;
    let prefs = profile.preferences.get_or_insert_with(Preferences::default);
    if let Some((_, entry)) = prefs.entries.iter_mut().find(|(k, _)| k == key) {
        entry.value = raw;
    } else {
        prefs.entries.push((
            key.to_owned(),
            PreferenceEntry {
                key: PreferenceKey::Known(spec.key),
                value: raw,
                units: String::new(),
                description: String::new(),
                comment: None,
            },
        ));
    }
    Ok(profile)
}

fn apply_unset_preference(mut profile: Profile, key: &str) -> Profile {
    if let Some(prefs) = profile.preferences.as_mut() {
        prefs.entries.retain(|(k, _)| k != key);
    }
    profile
}

fn apply_set_override(
    mut profile: Profile,
    sub_profile: &str,
    key: &str,
    value: &PreferenceValue,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let spec = lookup_preference_spec(key)?;
    let raw = coerce_value(&spec, value)?;
    let target = &mut profile.sub_profiles[sp_idx];
    let pref_key = PreferenceKey::Known(spec.key);
    let existing = target.rows.iter_mut().find(|r| match r {
        SubProfileRow::Override(o) => o.key == pref_key,
        SubProfileRow::Binding(_) => false,
    });
    if let Some(SubProfileRow::Override(o)) = existing {
        o.value = raw;
    } else {
        target
            .rows
            .push(SubProfileRow::Override(PreferenceOverride {
                key: pref_key,
                value: raw,
                comment: None,
            }));
    }
    Ok(profile)
}

fn apply_unset_override(
    mut profile: Profile,
    sub_profile: &str,
    key: &str,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let target_key = PreferenceSpec::for_id(key).map(|s| PreferenceKey::Known(s.key));
    let target = &mut profile.sub_profiles[sp_idx];
    target.rows.retain(|r| match r {
        SubProfileRow::Override(o) => target_key
            .as_ref()
            .map_or_else(|| o.key.as_csv() != key, |k| &o.key != k),
        SubProfileRow::Binding(_) => true,
    });
    Ok(profile)
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

    #[test]
    fn set_binding_rejects_unknown_input() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let err = apply(
            p,
            &[EditOp::SetBinding {
                sub_profile: "Main".into(),
                input: "lip_sof".into(),
                output: "kb_a".into(),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::UnknownInput { input, suggestions } => {
                assert_eq!(input, "lip_sof");
                assert!(
                    suggestions.iter().any(|s| s == "lip_soft"),
                    "expected 'lip_soft' in {suggestions:?}"
                );
            }
            other => panic!("expected UnknownInput, got {other:?}"),
        }
    }

    #[test]
    fn set_binding_rejects_unknown_output() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let err = apply(
            p,
            &[EditOp::SetBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_eter".into(),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::UnknownOutput {
                output,
                suggestions,
            } => {
                assert_eq!(output, "kb_eter");
                assert!(
                    suggestions.iter().any(|s| s == "kb_enter"),
                    "expected 'kb_enter' in {suggestions:?}"
                );
            }
            other => panic!("expected UnknownOutput, got {other:?}"),
        }
    }

    #[test]
    fn set_binding_rejects_missing_sub_profile() {
        let p = empty_profile();
        let err = apply(
            p,
            &[EditOp::SetBinding {
                sub_profile: "Ghost".into(),
                input: "lip_soft".into(),
                output: "kb_enter".into(),
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
    fn set_binding_appends_or_replaces_existing() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let out = apply(
            p,
            &[EditOp::SetBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_enter".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].bindings().count(), 1);
    }

    #[test]
    fn clear_binding_removes_existing() {
        use yoke_config::catalog::{Input, KbKey, Modifier, Output};
        use yoke_config::model::{Binding, SubProfileRow};
        let mut p = empty_profile();
        let mut sp = empty_sp("Main");
        sp.rows.push(SubProfileRow::Binding(Binding::new(
            Output::Keyboard(KbKey::Enter),
            Modifier::Normal,
            Some(Input::Lip { soft: true }),
        )));
        p.sub_profiles.push(sp);
        let out = apply(
            p,
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].bindings().count(), 0);
    }

    #[test]
    fn set_preference_rejects_unknown_key() {
        let p = empty_profile();
        let err = apply(
            p,
            &[EditOp::SetPreference {
                key: "Volum".into(),
                value: PreferenceValue::Number(55),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::UnknownPreference { key, suggestions } => {
                assert_eq!(key, "Volum");
                assert!(
                    suggestions.iter().any(|s| s == "volume"),
                    "expected 'volume' in {suggestions:?}"
                );
            }
            other => panic!("expected UnknownPreference, got {other:?}"),
        }
    }

    #[test]
    fn set_preference_rejects_out_of_range_number() {
        let p = empty_profile();
        let err = apply(
            p,
            &[EditOp::SetPreference {
                key: "volume".into(),
                value: PreferenceValue::Number(200),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::InvalidPreferenceValue { key, value, .. } => {
                assert_eq!(key, "volume");
                assert_eq!(value, "200");
            }
            other => panic!("expected InvalidPreferenceValue, got {other:?}"),
        }
    }

    #[test]
    fn set_preference_appends_when_missing() {
        let p = empty_profile();
        let out = apply(
            p,
            &[EditOp::SetPreference {
                key: "volume".into(),
                value: PreferenceValue::Number(55),
            }],
        )
        .unwrap();
        let prefs = out.preferences.unwrap();
        assert_eq!(prefs.entries.len(), 1);
        assert_eq!(prefs.entries[0].0, "volume");
        assert_eq!(prefs.entries[0].1.value, "55");
    }

    #[test]
    fn set_preference_replaces_existing_in_place() {
        use yoke_config::catalog::{KnownPreference, PreferenceKey};
        use yoke_config::model::{PreferenceEntry, Preferences};
        let mut p = empty_profile();
        p.preferences = Some(Preferences {
            entries: vec![(
                "volume".into(),
                PreferenceEntry {
                    key: PreferenceKey::Known(KnownPreference::Volume),
                    value: "30".into(),
                    units: String::new(),
                    description: String::new(),
                    comment: None,
                },
            )],
        });
        let out = apply(
            p,
            &[EditOp::SetPreference {
                key: "volume".into(),
                value: PreferenceValue::Number(55),
            }],
        )
        .unwrap();
        let prefs = out.preferences.unwrap();
        assert_eq!(prefs.entries.len(), 1);
        assert_eq!(prefs.entries[0].1.value, "55");
    }

    #[test]
    fn unset_preference_removes_entry() {
        use yoke_config::catalog::{KnownPreference, PreferenceKey};
        use yoke_config::model::{PreferenceEntry, Preferences};
        let mut p = empty_profile();
        p.preferences = Some(Preferences {
            entries: vec![(
                "volume".into(),
                PreferenceEntry {
                    key: PreferenceKey::Known(KnownPreference::Volume),
                    value: "30".into(),
                    units: String::new(),
                    description: String::new(),
                    comment: None,
                },
            )],
        });
        let out = apply(
            p,
            &[EditOp::UnsetPreference {
                key: "volume".into(),
            }],
        )
        .unwrap();
        assert!(out.preferences.unwrap().entries.is_empty());
    }

    #[test]
    fn set_override_appends_to_sub_profile_rows() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let out = apply(
            p,
            &[EditOp::SetOverride {
                sub_profile: "Main".into(),
                key: "volume".into(),
                value: PreferenceValue::Number(70),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].overrides().count(), 1);
    }

    #[test]
    fn unset_override_removes_matching_row() {
        use yoke_config::catalog::{KnownPreference, PreferenceKey};
        use yoke_config::model::{PreferenceOverride, SubProfileRow};
        let mut p = empty_profile();
        let mut sp = empty_sp("Main");
        sp.rows.push(SubProfileRow::Override(PreferenceOverride {
            key: PreferenceKey::Known(KnownPreference::Volume),
            value: "70".into(),
            comment: None,
        }));
        p.sub_profiles.push(sp);
        let out = apply(
            p,
            &[EditOp::UnsetOverride {
                sub_profile: "Main".into(),
                key: "volume".into(),
            }],
        )
        .unwrap();
        assert_eq!(out.sub_profiles[0].overrides().count(), 0);
    }

    #[test]
    fn batch_failure_leaves_input_profile_unchanged() {
        let mut p = empty_profile();
        p.sub_profiles.push(empty_sp("Main"));
        let snapshot = p.clone();
        let _ = apply(
            p.clone(),
            &[
                EditOp::SetTitle {
                    title: "New".into(),
                },
                EditOp::DeleteSubProfile {
                    name: "Ghost".into(),
                },
            ],
        )
        .unwrap_err();
        assert_eq!(p, snapshot);
    }

    #[test]
    fn set_binding_replace_preserves_modifier_and_comment() {
        use yoke_config::catalog::{Input, KbKey, Output};
        use yoke_config::model::{Binding, SubProfileRow};
        let mut p = empty_profile();
        let mut sp = empty_sp("Main");
        let mut b = Binding::new(
            Output::Keyboard(KbKey::Enter),
            Modifier::Toggle,
            Some(Input::Lip { soft: true }),
        );
        b.comment = Some("don't wipe me".to_owned());
        sp.rows.push(SubProfileRow::Binding(b));
        p.sub_profiles.push(sp);

        let out = apply(
            p,
            &[EditOp::SetBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
            }],
        )
        .unwrap();

        let row = out.sub_profiles[0]
            .rows
            .iter()
            .find_map(|r| match r {
                SubProfileRow::Binding(b) => Some(b),
                SubProfileRow::Override(_) => None,
            })
            .expect("expected the replaced binding to still be present");
        assert_eq!(row.output, Output::Keyboard(KbKey::A));
        assert!(matches!(row.modifier, Modifier::Toggle));
        assert_eq!(row.comment.as_deref(), Some("don't wipe me"));
    }
}
