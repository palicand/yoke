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

pub fn run_bindings(
    provider: &Arc<dyn VolumeProvider>,
    out: &Output,
    target: &str,
    sub_profile: Option<&str>,
) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    let parsed = yoke_config::parse(&bytes).context("parsing profile")?;
    let mut groups = group_bindings(&parsed.model);
    if let Some(filter) = sub_profile {
        if !groups.iter().any(|g| g.name == filter) {
            return Err(anyhow::Error::from(
                crate::error::CliError::SubProfileNameNotFound {
                    name: filter.to_string(),
                },
            ));
        }
        groups.retain(|g| g.name == filter);
    }
    out.emit(
        &serde_json::json!({
            "sub_profiles": groups.iter().map(|g| serde_json::json!({
                "name": g.name,
                "mode": g.mode,
                "channel": g.channel,
                "sub_mode": if g.sub_mode.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(g.sub_mode.to_string()) },
                "bindings": g.bindings.iter().map(|b| serde_json::json!({
                    "input": b.input, "output": b.output, "modifier": b.modifier,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        |w| {
            // The default modifier is suppressed so the common case stays clean
            // (mirrors the binding-row pill convention); derive it from the typed
            // default rather than hardcoding the keyword.
            let default_modifier = yoke_config::catalog::Modifier::Normal.to_csv();
            for (i, g) in groups.iter().enumerate() {
                if i > 0 {
                    writeln!(w)?;
                }
                writeln!(w, "{} (mode={} channel={})", g.name, g.mode, g.channel)?;
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
        },
    )
}

/// `overridden` is set whenever the sub-profile carries an override for this key,
/// regardless of whether a top-level entry also exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePrefValue {
    pub value: String,
    pub overridden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePreferences {
    pub top_level: BTreeMap<String, String>,
    pub per_sub_profile: Vec<(String, BTreeMap<String, EffectivePrefValue>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPreferences {
    pub top_level: BTreeMap<String, String>,
    pub per_sub_profile_overrides: Vec<(String, BTreeMap<String, String>)>,
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
    sub_profile: Option<&str>,
    raw: bool,
) -> Result<()> {
    let t = crate::target::Target::classify(target);
    let bytes = t.read_bytes(provider.as_ref())?;
    let parsed = yoke_config::parse(&bytes).context("parsing profile")?;
    if let Some(filter) = sub_profile
        && !parsed
            .model
            .sub_profiles
            .iter()
            .any(|sp| sp.header.profile_name == filter)
    {
        return Err(anyhow::Error::from(
            crate::error::CliError::SubProfileNameNotFound {
                name: filter.to_string(),
            },
        ));
    }
    if raw {
        let mut data = raw_preferences(&parsed.model);
        if let Some(filter) = sub_profile {
            data.per_sub_profile_overrides.retain(|(n, _)| n == filter);
        }
        emit_raw_preferences(out, &data)
    } else {
        let mut data = effective_preferences(&parsed.model);
        if let Some(filter) = sub_profile {
            data.per_sub_profile.retain(|(n, _)| n == filter);
        }
        emit_effective_preferences(out, &data)
    }
}

fn emit_effective_preferences(out: &Output, data: &EffectivePreferences) -> Result<()> {
    out.emit(
        &serde_json::json!({
            "top_level": data.top_level,
            "sub_profiles": data.per_sub_profile.iter().map(|(name, prefs)| serde_json::json!({
                "name": name,
                "preferences": prefs.iter().map(|(k, v)| (k.clone(), serde_json::json!({
                    "value": v.value, "overridden": v.overridden,
                }))).collect::<serde_json::Map<_, _>>(),
            })).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "Top-level:")?;
            for (k, v) in &data.top_level {
                writeln!(w, "  {k:<25} {v}")?;
            }
            for (name, prefs) in &data.per_sub_profile {
                writeln!(w)?;
                writeln!(w, "{name}:")?;
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

fn emit_raw_preferences(out: &Output, data: &RawPreferences) -> Result<()> {
    out.emit(
        &serde_json::json!({
            "top_level": data.top_level,
            "sub_profiles": data.per_sub_profile_overrides.iter().map(|(name, ov)| serde_json::json!({
                "name": name,
                "overrides": ov,
            })).collect::<Vec<_>>(),
        }),
        |w| {
            writeln!(w, "Top-level:")?;
            for (k, v) in &data.top_level {
                writeln!(w, "  {k:<25} {v}")?;
            }
            for (name, ov) in &data.per_sub_profile_overrides {
                writeln!(w)?;
                writeln!(w, "{name} (overrides):")?;
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
