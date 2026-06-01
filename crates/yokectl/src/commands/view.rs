use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use yoke_config::catalog::Input;
use yoke_config::model::Profile;
use yoke_volume::VolumeProvider;

use crate::output::Output;

/// One sub-profile worth of bindings, ready to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedSubProfile<'a> {
    pub name: &'a str,
    pub mode: String,
    pub channel: String,
    pub sub_mode: &'a str,
    pub bindings: Vec<GroupedBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupedBinding {
    pub input: Option<String>,
    pub output: String,
    pub modifier: String,
}

/// Sub-profiles are returned in declaration order; bindings within each group
/// are sorted by their rendered input for deterministic output.
#[must_use]
pub fn group_bindings(profile: &Profile) -> Vec<GroupedSubProfile<'_>> {
    profile
        .sub_profiles
        .iter()
        .map(|sp| {
            let mut bindings: Vec<GroupedBinding> = sp
                .bindings()
                .map(|b| GroupedBinding {
                    input: b.input.as_ref().map(Input::to_csv),
                    output: b.output.to_csv(),
                    modifier: b.modifier.to_csv(),
                })
                .collect();
            bindings.sort_by(|a, b| a.input.cmp(&b.input));
            GroupedSubProfile {
                name: sp.header.profile_name.as_str(),
                mode: sp.header.mode.canonical_csv(),
                channel: sp.header.channel.canonical_csv().to_string(),
                sub_mode: sp.header.sub_mode.as_str(),
                bindings,
            }
        })
        .collect()
}

/// Builds the JSON envelope for `bindings`, carrying each group's true index so a
/// filtered query keeps the original `sub_profile_index` rather than re-counting from 0.
pub(crate) fn bindings_json(
    groups_with_index: &[(usize, &GroupedSubProfile)],
) -> serde_json::Value {
    serde_json::json!({
        "sub_profiles": groups_with_index.iter().map(|(i, g)| serde_json::json!({
            "sub_profile_index": i,
            "name": g.name,
            "mode": g.mode,
            "channel": g.channel,
            "sub_mode": if g.sub_mode.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(g.sub_mode.to_string()) },
            "bindings": g.bindings.iter().map(|b| serde_json::json!({
                "input": b.input, "output": b.output, "modifier": b.modifier,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// Numbers items by their declaration-order index — the address every edit op uses and that
/// `bindings`/`preferences` show the user — then, when `idx` is set, keeps only that index.
/// An out-of-range index refuses (the index the user passes must match what they were shown)
/// rather than silently emitting nothing.
fn number_and_filter<T>(
    items: &[T],
    idx: Option<usize>,
) -> Result<Vec<(usize, &T)>, crate::error::CliError> {
    let mut numbered: Vec<(usize, &T)> = items.iter().enumerate().collect();
    if let Some(i) = idx {
        if i >= numbered.len() {
            return Err(crate::error::CliError::SubProfileIndexOutOfRange {
                index: i,
                len: numbered.len(),
            });
        }
        numbered.retain(|(j, _)| *j == i);
    }
    Ok(numbered)
}

pub fn run_bindings(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sub_profile: Option<usize>,
) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    let parsed = yoke_config::parse(&bytes).context("parsing profile")?;
    let groups = group_bindings(&parsed.model);
    let numbered = number_and_filter(&groups, sub_profile)?;
    out.emit(&bindings_json(&numbered), |w| {
        // The default modifier is suppressed so the common case stays clean
        // (mirrors the binding-row pill convention); derive it from the typed
        // default rather than hardcoding the keyword.
        let default_modifier = yoke_config::catalog::Modifier::Normal.to_csv();
        for (pos, (i, g)) in numbered.iter().enumerate() {
            if pos > 0 {
                writeln!(w)?;
            }
            writeln!(
                w,
                "[#{i}] {} (mode={} channel={})",
                g.name, g.mode, g.channel
            )?;
            if g.bindings.is_empty() {
                writeln!(w, "  (no bindings)")?;
            } else {
                for b in &g.bindings {
                    let input = b.input.as_deref().unwrap_or("(none)");
                    if b.modifier == default_modifier {
                        writeln!(w, "  {:<15} -> {}", input, b.output)?;
                    } else {
                        writeln!(w, "  {:<15} -> {} [{}]", input, b.output, b.modifier)?;
                    }
                }
            }
        }
        Ok(())
    })
}

/// `overridden` is set whenever the sub-profile carries an override for this key,
/// regardless of whether a top-level entry also exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePrefValue {
    pub value: String,
    pub overridden: bool,
}

/// One sub-profile's resolved preferences, paired with its display name (positional index
/// is the `Vec` position).
type EffectiveEntry = (String, BTreeMap<String, EffectivePrefValue>);
/// One sub-profile's raw overrides, paired with its display name.
type RawEntry = (String, BTreeMap<String, String>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePreferences {
    pub top_level: BTreeMap<String, String>,
    pub per_sub_profile: Vec<EffectiveEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPreferences {
    pub top_level: BTreeMap<String, String>,
    pub per_sub_profile_overrides: Vec<RawEntry>,
}

#[must_use]
pub fn effective_preferences(profile: &Profile) -> EffectivePreferences {
    // Key by the normalized id (`PreferenceKey::as_csv`) rather than the raw CSV cell so
    // the join against per-sub-profile overrides (also keyed by `as_csv`) cannot miss when
    // the top-level id carries padding or non-canonical casing.
    let top_level: BTreeMap<String, String> = profile
        .preferences
        .as_ref()
        .map(|prefs| {
            prefs
                .entries
                .iter()
                .map(|(_, e)| (e.key.as_csv(), e.value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let per_sub_profile = profile
        .sub_profiles
        .iter()
        .map(|sp| {
            let overrides: BTreeMap<String, String> = sp
                .overrides()
                .map(|o| (o.key.as_csv(), o.value.clone()))
                .collect();
            let mut resolved: BTreeMap<String, EffectivePrefValue> = BTreeMap::new();
            for (k, v) in &top_level {
                resolved.insert(
                    k.clone(),
                    EffectivePrefValue {
                        value: overrides.get(k).cloned().unwrap_or_else(|| v.clone()),
                        overridden: overrides.contains_key(k),
                    },
                );
            }
            for (k, v) in &overrides {
                resolved
                    .entry(k.clone())
                    .or_insert_with(|| EffectivePrefValue {
                        value: v.clone(),
                        overridden: true,
                    });
            }
            (sp.header.profile_name.clone(), resolved)
        })
        .collect();
    EffectivePreferences {
        top_level,
        per_sub_profile,
    }
}

#[must_use]
pub fn raw_preferences(profile: &Profile) -> RawPreferences {
    let top_level: BTreeMap<String, String> = profile
        .preferences
        .as_ref()
        .map(|prefs| {
            prefs
                .entries
                .iter()
                .map(|(k, e)| (k.clone(), e.value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let per_sub_profile_overrides = profile
        .sub_profiles
        .iter()
        .map(|sp| {
            let overrides: BTreeMap<String, String> = sp
                .overrides()
                .map(|o| (o.key.as_csv(), o.value.clone()))
                .collect();
            (sp.header.profile_name.clone(), overrides)
        })
        .collect();
    RawPreferences {
        top_level,
        per_sub_profile_overrides,
    }
}

pub fn run_preferences(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sub_profile: Option<usize>,
    raw: bool,
) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    let parsed = yoke_config::parse(&bytes).context("parsing profile")?;
    if raw {
        let data = raw_preferences(&parsed.model);
        let numbered = number_and_filter(&data.per_sub_profile_overrides, sub_profile)?;
        emit_raw_preferences(out, &data.top_level, &numbered)
    } else {
        let data = effective_preferences(&parsed.model);
        let numbered = number_and_filter(&data.per_sub_profile, sub_profile)?;
        emit_effective_preferences(out, &data.top_level, &numbered)
    }
}

// `sub_profile_index` is carried alongside each block (and shown as `[#i]`) so a reader can map
// an override back to the index they must pass to set-override/unset-override — names are not
// unique, so the index is the only unambiguous handle.
fn emit_effective_preferences(
    out: &Output,
    top_level: &BTreeMap<String, String>,
    subs: &[(usize, &EffectiveEntry)],
) -> Result<()> {
    out.emit(
        &serde_json::json!({
            "top_level": top_level,
            "sub_profiles": subs.iter().map(|(i, (name, prefs))| serde_json::json!({
                "sub_profile_index": i,
                "name": name,
                "preferences": prefs.iter().map(|(k, v)| (k.clone(), serde_json::json!({
                    "value": v.value, "overridden": v.overridden,
                }))).collect::<serde_json::Map<_, _>>(),
            })).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "Top-level:")?;
            for (k, v) in top_level {
                writeln!(w, "  {k:<25} {v}")?;
            }
            for (i, (name, prefs)) in subs {
                writeln!(w)?;
                writeln!(w, "[#{i}] {name}:")?;
                for (k, v) in prefs {
                    if v.overridden {
                        writeln!(w, "  {k:<25} {:<15} [override]", v.value)?;
                    } else {
                        writeln!(w, "  {k:<25} {}", v.value)?;
                    }
                }
            }
            Ok(())
        },
    )
}

fn emit_raw_preferences(
    out: &Output,
    top_level: &BTreeMap<String, String>,
    subs: &[(usize, &RawEntry)],
) -> Result<()> {
    out.emit(
        &serde_json::json!({
            "top_level": top_level,
            "sub_profiles": subs.iter().map(|(i, (name, ov))| serde_json::json!({
                "sub_profile_index": i,
                "name": name,
                "overrides": ov,
            })).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "Top-level:")?;
            for (k, v) in top_level {
                writeln!(w, "  {k:<25} {v}")?;
            }
            for (i, (name, ov)) in subs {
                writeln!(w)?;
                writeln!(w, "[#{i}] {name} (overrides):")?;
                if ov.is_empty() {
                    writeln!(w, "  (none)")?;
                } else {
                    for (k, v) in ov {
                        writeln!(w, "  {k:<25} {v}")?;
                    }
                }
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_config::catalog::{Channel, Input, Modifier, Output, SubProfileMode};
    use yoke_config::model::{Binding, SubProfile, SubProfileHeader, SubProfileRow, TopLine};

    fn profile_with_one_binding() -> Profile {
        Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
                width: 4,
            },
            sub_profiles: vec![SubProfile {
                header: SubProfileHeader {
                    profile_name: "Main".into(),
                    mode: SubProfileMode::Mouse,
                    sub_mode: String::new(),
                    channel: Channel::Usb,
                    column_header_label: "Output or Function".into(),
                },
                rows: vec![SubProfileRow::Binding(Binding::new(
                    Output::Touch,
                    Modifier::Normal,
                    Some(Input::Lip { soft: false }),
                ))],
            }],
            preferences: None,
            infrared: vec![],
        }
    }

    use yoke_config::catalog::PreferenceKey;
    use yoke_config::model::{PreferenceEntry, PreferenceOverride, Preferences};

    fn profile_with_prefs_and_override() -> Profile {
        Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
                width: 4,
            },
            sub_profiles: vec![SubProfile {
                header: SubProfileHeader {
                    profile_name: "Main".into(),
                    mode: SubProfileMode::Mouse,
                    sub_mode: String::new(),
                    channel: Channel::Usb,
                    column_header_label: "Output or Function".into(),
                },
                rows: vec![SubProfileRow::Override(PreferenceOverride {
                    key: PreferenceKey::from_csv("volume"),
                    value: "70".into(),
                    comment: None,
                })],
            }],
            preferences: Some(Preferences {
                entries: vec![(
                    "volume".into(),
                    PreferenceEntry {
                        key: PreferenceKey::from_csv("volume"),
                        value: "55".into(),
                        units: String::new(),
                        description: String::new(),
                        comment: None,
                    },
                )],
            }),
            infrared: vec![],
        }
    }

    fn profile_with_padded_pref_and_override() -> Profile {
        let mut p = profile_with_prefs_and_override();
        p.preferences = Some(Preferences {
            entries: vec![(
                " volume ".into(),
                PreferenceEntry {
                    key: PreferenceKey::from_csv(" volume "),
                    value: "55".into(),
                    units: String::new(),
                    description: String::new(),
                    comment: None,
                },
            )],
        });
        p
    }

    #[test]
    fn effective_normalizes_padded_top_level_key_against_override() {
        let p = profile_with_padded_pref_and_override();
        let eff = effective_preferences(&p);
        let (_, sp) = &eff.per_sub_profile[0];
        assert_eq!(
            sp.len(),
            1,
            "padded top-level id should normalize to one key"
        );
        let v = sp.get("volume").expect("normalized volume key present");
        assert_eq!(v.value, "70");
        assert!(v.overridden);
    }

    #[test]
    fn effective_marks_override_when_present() {
        let p = profile_with_prefs_and_override();
        let eff = effective_preferences(&p);
        assert_eq!(eff.top_level.get("volume").map(String::as_str), Some("55"));
        let (_, sp) = &eff.per_sub_profile[0];
        let v = sp.get("volume").expect("Main has volume override");
        assert_eq!(v.value, "70");
        assert!(v.overridden);
    }

    #[test]
    fn raw_skips_resolution() {
        let p = profile_with_prefs_and_override();
        let raw = raw_preferences(&p);
        assert_eq!(raw.top_level.get("volume").map(String::as_str), Some("55"));
        let (_, ov) = &raw.per_sub_profile_overrides[0];
        assert_eq!(ov.get("volume").map(String::as_str), Some("70"));
    }

    fn profile_with_inputless_binding() -> Profile {
        Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
                width: 4,
            },
            sub_profiles: vec![SubProfile {
                header: SubProfileHeader {
                    profile_name: "Main".into(),
                    mode: SubProfileMode::Mouse,
                    sub_mode: String::new(),
                    channel: Channel::Usb,
                    column_header_label: "Output or Function".into(),
                },
                rows: vec![SubProfileRow::Binding(Binding::new(
                    Output::Touch,
                    Modifier::Normal,
                    None,
                ))],
            }],
            preferences: None,
            infrared: vec![],
        }
    }

    #[test]
    fn binding_without_input_has_no_input_value() {
        let p = profile_with_inputless_binding();
        let g = group_bindings(&p);
        assert_eq!(g[0].bindings[0].input, None);
    }

    #[test]
    fn groups_single_subprofile_with_single_binding() {
        let p = profile_with_one_binding();
        let g = group_bindings(&p);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "Main");
        assert_eq!(g[0].bindings.len(), 1);
    }

    #[test]
    fn grouped_binding_carries_modifier() {
        let mut p = profile_with_one_binding();
        if let SubProfileRow::Binding(b) = &mut p.sub_profiles[0].rows[0] {
            b.modifier = Modifier::DelayOn { ms: Some(250) };
        }
        let g = group_bindings(&p);
        assert_eq!(g[0].bindings[0].modifier, "delay_on 250");
    }

    fn profile_with_three_sub_profiles() -> Profile {
        let sub = |name: &str| SubProfile {
            header: SubProfileHeader {
                profile_name: name.into(),
                mode: SubProfileMode::Mouse,
                sub_mode: String::new(),
                channel: Channel::Usb,
                column_header_label: "Output or Function".into(),
            },
            rows: vec![SubProfileRow::Binding(Binding::new(
                Output::Touch,
                Modifier::Normal,
                Some(Input::Lip { soft: false }),
            ))],
        };
        Profile {
            top_line: TopLine {
                label: "QuadStick Configuration".into(),
                version: "Version 1.4".into(),
                source: String::new(),
                title: "Default".into(),
                trailing_cells: vec![],
                width: 4,
            },
            sub_profiles: vec![sub("Zero"), sub("One"), sub("Two")],
            preferences: None,
            infrared: vec![],
        }
    }

    #[test]
    fn bindings_json_numbers_every_group() {
        let p = profile_with_three_sub_profiles();
        let groups = group_bindings(&p);
        let numbered: Vec<(usize, &GroupedSubProfile)> = groups.iter().enumerate().collect();
        let v = bindings_json(&numbered);
        let subs = v["sub_profiles"].as_array().expect("array");
        assert_eq!(subs.len(), 3);
        for (i, sub) in subs.iter().enumerate() {
            assert_eq!(sub["sub_profile_index"], serde_json::json!(i));
        }
    }

    #[test]
    fn bindings_json_filtered_keeps_true_index() {
        let p = profile_with_three_sub_profiles();
        let groups = group_bindings(&p);
        // Simulate `--sub-profile 1`: number first, then keep only index 1.
        let numbered: Vec<(usize, &GroupedSubProfile)> =
            groups.iter().enumerate().filter(|(i, _)| *i == 1).collect();
        let v = bindings_json(&numbered);
        let subs = v["sub_profiles"].as_array().expect("array");
        assert_eq!(subs.len(), 1);
        assert_eq!(
            subs[0]["sub_profile_index"],
            serde_json::json!(1),
            "filtered group must keep its original index, not re-count to 0"
        );
        assert_eq!(subs[0]["name"], serde_json::json!("One"));
    }

    #[test]
    fn empty_profile_returns_empty_vec() {
        let p = Profile {
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
        };
        assert!(group_bindings(&p).is_empty());
    }
}
