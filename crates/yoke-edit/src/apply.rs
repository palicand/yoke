use yoke_config::catalog::{
    Input, Modifier, Output, PreferenceKey, PreferenceSpec, PreferenceValueKind,
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
        EditOp::AddBinding {
            sub_profile,
            input,
            output,
            modifier,
        } => apply_add_binding(profile, sub_profile, input, output, modifier.as_deref()),
        EditOp::UpdateBinding {
            sub_profile,
            input,
            output,
            modifier,
        } => apply_update_binding(profile, sub_profile, input, output, modifier),
        EditOp::ClearBinding {
            sub_profile,
            input,
            modifier,
        } => apply_clear_binding(profile, sub_profile, input, modifier.as_deref()),
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

// A binding row is identified by its (input, modifier) pair, which maps to exactly one
// output. `add` appends a new pair (POST), `update` mutates the single matching row (PUT),
// `clear` deletes by input or by the unique (input, modifier) pair (DELETE).
fn apply_add_binding(
    mut profile: Profile,
    sub_profile: &str,
    input: &str,
    output: &str,
    modifier: Option<&str>,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let parsed_input = parse_input(input)?;
    let parsed_output = parse_output(output)?;
    let parsed_modifier = match modifier {
        Some(m) => parse_modifier(m)?,
        None => Modifier::Normal,
    };
    let target = &mut profile.sub_profiles[sp_idx];
    // (input, modifier) maps to exactly one output; refuse to create a second mapping.
    let existing_output = target.bindings().find_map(|b| {
        (b.input.as_ref() == Some(&parsed_input) && b.modifier == parsed_modifier)
            .then(|| b.output.to_csv())
    });
    if let Some(output) = existing_output {
        return Err(EditError::BindingExists {
            sub_profile: sub_profile.to_owned(),
            input: input.to_owned(),
            modifier: parsed_modifier.to_csv(),
            output,
        });
    }
    target.rows.push(SubProfileRow::Binding(Binding::new(
        parsed_output,
        parsed_modifier,
        Some(parsed_input),
    )));
    Ok(profile)
}

fn apply_clear_binding(
    mut profile: Profile,
    sub_profile: &str,
    input: &str,
    modifier: Option<&str>,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let parsed_input = parse_input(input)?;
    let parsed_modifier = match modifier {
        Some(m) => Some(parse_modifier(m)?),
        None => None,
    };
    let target = &mut profile.sub_profiles[sp_idx];
    let before = target.rows.len();
    target.rows.retain(|r| match r {
        SubProfileRow::Binding(b) if b.input.as_ref() == Some(&parsed_input) => {
            // No modifier given: remove every row for this input (keep none). Modifier
            // given: keep rows whose modifier differs, dropping the unique matching one.
            parsed_modifier.as_ref().is_some_and(|m| b.modifier != *m)
        }
        _ => true,
    });
    if target.rows.len() == before {
        return Err(EditError::BindingNotFound {
            sub_profile: sub_profile.to_owned(),
            input: input.to_owned(),
        });
    }
    Ok(profile)
}

fn parse_modifier(raw: &str) -> Result<Modifier, EditError> {
    match Modifier::from_csv(raw) {
        Some(m) if !matches!(m, Modifier::Unknown { .. }) => Ok(m),
        _ => {
            // `from_csv` round-trips both an unrecognized keyword and a recognized keyword
            // carrying bad/extra arguments (e.g. "delay_on abc") to Unknown. Split the two on
            // the leading token, scored against Modifier::KEYWORDS (which holds keywords, not
            // full phrases): a known keyword means the arguments are at fault, so report that
            // directly instead of suggesting the keyword back to itself; an unknown keyword
            // gets the usual edit-distance suggestions (scoring the whole phrase would exceed
            // the cap and surface none).
            let keyword = raw.split_whitespace().next().unwrap_or(raw);
            if Modifier::KEYWORDS.contains(&keyword) {
                Err(EditError::InvalidModifierArguments {
                    keyword: keyword.to_owned(),
                    modifier: raw.to_owned(),
                })
            } else {
                Err(EditError::UnknownModifier {
                    modifier: raw.to_owned(),
                    suggestions: suggestions(keyword, Modifier::KEYWORDS.iter().copied()),
                })
            }
        }
    }
}

fn apply_update_binding(
    mut profile: Profile,
    sub_profile: &str,
    input: &str,
    output: &str,
    modifier: &str,
) -> Result<Profile, EditError> {
    let sp_idx = sub_profile_index(&profile, sub_profile)?;
    let parsed_input = parse_input(input)?;
    let parsed_output = parse_output(output)?;
    let parsed_modifier = parse_modifier(modifier)?;
    let target = &mut profile.sub_profiles[sp_idx];

    // Anchor the target row by whichever of (input, modifier) / (input, output) the user
    // already has; change the other field. (input, modifier) is unique by the add invariant;
    // (input, output) may not be, so a multi-match there is ambiguous.
    let mut exact = false;
    let mut by_modifier: Vec<usize> = Vec::new();
    let mut by_output: Vec<usize> = Vec::new();
    for (i, r) in target.rows.iter().enumerate() {
        if let SubProfileRow::Binding(b) = r {
            if b.input.as_ref() != Some(&parsed_input) {
                continue;
            }
            let same_modifier = b.modifier == parsed_modifier;
            let same_output = b.output == parsed_output;
            if same_modifier && same_output {
                exact = true;
            } else if same_modifier {
                by_modifier.push(i);
            } else if same_output {
                by_output.push(i);
            }
        }
    }

    if exact {
        return Ok(profile); // the requested binding already exists verbatim
    }
    match (by_modifier.as_slice(), by_output.as_slice()) {
        ([], []) => Err(EditError::BindingNotFound {
            sub_profile: sub_profile.to_owned(),
            input: input.to_owned(),
        }),
        ([i], []) => {
            if let SubProfileRow::Binding(b) = &mut target.rows[*i] {
                b.output = parsed_output;
            }
            Ok(profile)
        }
        ([], [i]) => {
            if let SubProfileRow::Binding(b) = &mut target.rows[*i] {
                b.modifier = parsed_modifier;
            }
            Ok(profile)
        }
        _ => Err(EditError::AmbiguousBinding {
            sub_profile: sub_profile.to_owned(),
            input: input.to_owned(),
            output: output.to_owned(),
        }),
    }
}

fn parse_input(raw: &str) -> Result<Input, EditError> {
    match Input::from_csv(raw) {
        Input::Unknown(_) => {
            let names: Vec<String> = Input::all_csv_names().collect();
            Err(EditError::UnknownInput {
                input: raw.to_owned(),
                suggestions: suggestions(raw, names.iter().map(String::as_str)),
            })
        }
        ok => Ok(ok),
    }
}

fn parse_output(raw: &str) -> Result<Output, EditError> {
    match Output::from_csv(raw) {
        Output::Unknown(_) => {
            let names: Vec<String> = Output::all_csv_names().collect();
            Err(EditError::UnknownOutput {
                output: raw.to_owned(),
                suggestions: suggestions(raw, names.iter().map(String::as_str)),
            })
        }
        ok => Ok(ok),
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

    use yoke_config::catalog::KbKey;

    fn binding(output: Output, modifier: Modifier, input: Input) -> SubProfileRow {
        SubProfileRow::Binding(Binding::new(output, modifier, Some(input)))
    }

    fn main_with(rows: Vec<SubProfileRow>) -> Profile {
        let mut p = empty_profile();
        let mut sp = empty_sp("Main");
        sp.rows = rows;
        p.sub_profiles.push(sp);
        p
    }

    fn bindings_of(p: &Profile) -> Vec<&Binding> {
        p.sub_profiles[0].bindings().collect()
    }

    // --- add-binding (POST) ---

    #[test]
    fn add_binding_creates_when_input_unbound() {
        let out = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap();
        let bs = bindings_of(&out);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].output, Output::Keyboard(KbKey::A));
        assert_eq!(bs[0].modifier, Modifier::Toggle);
        assert_eq!(bs[0].input, Some(Input::Lip { soft: true }));
    }

    #[test]
    fn add_binding_defaults_modifier_to_normal_when_omitted() {
        let out = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: None,
            }],
        )
        .unwrap();
        assert_eq!(bindings_of(&out)[0].modifier, Modifier::Normal);
    }

    #[test]
    fn add_binding_appends_parallel_output_for_same_input() {
        // existing input, new (modifier, output) -> add a second (chord) row
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::Enter),
            Modifier::Normal,
            Input::Lip { soft: true },
        )]);
        let out = apply(
            p,
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap();
        assert_eq!(bindings_of(&out).len(), 2);
    }

    #[test]
    fn add_binding_allows_duplicate_output_with_distinct_modifier() {
        // same (input, output), different modifier -> allowed (worst case is a redundant dup)
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Normal,
            Input::Lip { soft: true },
        )]);
        let out = apply(
            p,
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap();
        assert_eq!(bindings_of(&out).len(), 2);
    }

    #[test]
    fn add_binding_rejects_existing_input_modifier_pair() {
        // (input, modifier) already maps to an output -> conflict, regardless of the new output
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Toggle,
            Input::Lip { soft: true },
        )]);
        let err = apply(
            p,
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_b".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::BindingExists {
                input,
                modifier,
                output,
                ..
            } => {
                assert_eq!(input, "lip_soft");
                assert_eq!(modifier, "toggle");
                assert_eq!(output, "kb_a"); // reports the output it already maps to
            }
            other => panic!("expected BindingExists, got {other:?}"),
        }
    }

    #[test]
    fn add_binding_rejects_unknown_input() {
        let err = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_sof".into(),
                output: "kb_a".into(),
                modifier: None,
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
    fn add_binding_rejects_unknown_output() {
        let err = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_eter".into(),
                modifier: None,
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
    fn add_binding_rejects_unknown_modifier_with_suggestion() {
        let err = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: Some("togle".into()),
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::UnknownModifier {
                modifier,
                suggestions,
            } => {
                assert_eq!(modifier, "togle");
                assert!(
                    suggestions.iter().any(|s| s == "toggle"),
                    "expected 'toggle' in {suggestions:?}"
                );
            }
            other => panic!("expected UnknownModifier, got {other:?}"),
        }
    }

    #[test]
    fn add_binding_reports_invalid_arguments_for_known_keyword() {
        // A real keyword with a bad argument is an argument error, not an unknown modifier:
        // it must not echo the keyword back as a "did you mean" suggestion.
        let err = apply(
            main_with(vec![]),
            &[EditOp::AddBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: Some("delay_on abc".into()),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::InvalidModifierArguments {
                keyword: "delay_on".into(),
                modifier: "delay_on abc".into(),
            }
        );
    }

    #[test]
    fn add_binding_rejects_missing_sub_profile() {
        let err = apply(
            empty_profile(),
            &[EditOp::AddBinding {
                sub_profile: "Ghost".into(),
                input: "lip_soft".into(),
                output: "kb_enter".into(),
                modifier: None,
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

    // --- update-binding (PUT) ---

    #[test]
    fn update_binding_changes_output_anchored_on_modifier() {
        // (input, modifier) exists, output is new -> change that row's output
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Toggle,
            Input::Lip { soft: true },
        )]);
        let out = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_b".into(),
                modifier: "toggle".into(),
            }],
        )
        .unwrap();
        let bs = bindings_of(&out);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].output, Output::Keyboard(KbKey::B));
        assert_eq!(bs[0].modifier, Modifier::Toggle);
    }

    #[test]
    fn update_binding_changes_modifier_anchored_on_output() {
        // (input, output) exists, modifier is new -> change that row's modifier (the old set-modifier use case)
        let mut b = Binding::new(
            Output::Keyboard(KbKey::A),
            Modifier::Normal,
            Some(Input::Lip { soft: true }),
        );
        b.comment = Some("keep me".to_owned());
        let p = main_with(vec![SubProfileRow::Binding(b)]);
        let out = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: "delay_on 250".into(),
            }],
        )
        .unwrap();
        let bs = bindings_of(&out);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].modifier, Modifier::DelayOn { ms: Some(250) });
        assert_eq!(bs[0].output, Output::Keyboard(KbKey::A)); // output untouched
        assert_eq!(bs[0].comment.as_deref(), Some("keep me")); // comment preserved
    }

    #[test]
    fn update_binding_is_noop_on_exact_triple() {
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Toggle,
            Input::Lip { soft: true },
        )]);
        let snapshot = p.clone();
        let out = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: "toggle".into(),
            }],
        )
        .unwrap();
        assert_eq!(out, snapshot);
    }

    #[test]
    fn update_binding_rejects_unbound_input() {
        let err = apply(
            main_with(vec![]),
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: "toggle".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::BindingNotFound {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
            }
        );
    }

    #[test]
    fn update_binding_rejects_when_no_anchor_matches() {
        // input is bound, but neither (input, modifier) nor (input, output) matches
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Normal,
            Input::Lip { soft: true },
        )]);
        let err = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_b".into(),
                modifier: "toggle".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::BindingNotFound {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
            }
        );
    }

    #[test]
    fn update_binding_rejects_ambiguous_output_match() {
        // two rows share (input, output); a new modifier cannot pick one
        let p = main_with(vec![
            binding(
                Output::Keyboard(KbKey::A),
                Modifier::Normal,
                Input::Lip { soft: true },
            ),
            binding(
                Output::Keyboard(KbKey::A),
                Modifier::Toggle,
                Input::Lip { soft: true },
            ),
        ]);
        let err = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
                modifier: "delay_on 250".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::AmbiguousBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_a".into(),
            }
        );
    }

    #[test]
    fn update_binding_rejects_when_anchors_match_different_rows() {
        // (input, modifier) matches one row, (input, output) another -> ambiguous intent
        let p = main_with(vec![
            binding(
                Output::Keyboard(KbKey::A),
                Modifier::Toggle,
                Input::Lip { soft: true },
            ),
            binding(
                Output::Keyboard(KbKey::B),
                Modifier::Normal,
                Input::Lip { soft: true },
            ),
        ]);
        let err = apply(
            p,
            &[EditOp::UpdateBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_b".into(),
                modifier: "toggle".into(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::AmbiguousBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                output: "kb_b".into(),
            }
        );
    }

    // --- clear-binding (DELETE) ---

    #[test]
    fn clear_binding_removes_all_rows_for_input_when_modifier_omitted() {
        let p = main_with(vec![
            binding(
                Output::Keyboard(KbKey::A),
                Modifier::Normal,
                Input::Lip { soft: true },
            ),
            binding(
                Output::Keyboard(KbKey::B),
                Modifier::Toggle,
                Input::Lip { soft: true },
            ),
            binding(
                Output::Keyboard(KbKey::Enter),
                Modifier::Normal,
                Input::Center,
            ),
        ]);
        let out = apply(
            p,
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                modifier: None,
            }],
        )
        .unwrap();
        let bs = bindings_of(&out);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].input, Some(Input::Center));
    }

    #[test]
    fn clear_binding_removes_only_matching_modifier_row() {
        let p = main_with(vec![
            binding(
                Output::Keyboard(KbKey::A),
                Modifier::Normal,
                Input::Lip { soft: true },
            ),
            binding(
                Output::Keyboard(KbKey::B),
                Modifier::Toggle,
                Input::Lip { soft: true },
            ),
        ]);
        let out = apply(
            p,
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap();
        let bs = bindings_of(&out);
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].modifier, Modifier::Normal);
    }

    #[test]
    fn clear_binding_rejects_unbound_input() {
        // catalog-valid input with no binding -> BindingNotFound, not UnknownInput
        let err = apply(
            main_with(vec![]),
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                modifier: None,
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::BindingNotFound {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
            }
        );
    }

    #[test]
    fn clear_binding_rejects_missing_modifier_row() {
        let p = main_with(vec![binding(
            Output::Keyboard(KbKey::A),
            Modifier::Normal,
            Input::Lip { soft: true },
        )]);
        let err = apply(
            p,
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
                modifier: Some("toggle".into()),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            EditError::BindingNotFound {
                sub_profile: "Main".into(),
                input: "lip_soft".into(),
            }
        );
    }

    #[test]
    fn clear_binding_rejects_unknown_input_parse() {
        let err = apply(
            main_with(vec![]),
            &[EditOp::ClearBinding {
                sub_profile: "Main".into(),
                input: "lip_sof".into(),
                modifier: None,
            }],
        )
        .unwrap_err();
        match err.error {
            EditError::UnknownInput { input, .. } => assert_eq!(input, "lip_sof"),
            other => panic!("expected UnknownInput, got {other:?}"),
        }
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
}
