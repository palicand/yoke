use yoke_config::catalog::Input;
use yoke_config::model::Profile;

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
    pub input: String,
    pub output: String,
}

/// Walks a Profile and returns bindings grouped by sub-profile in declaration order;
/// bindings inside each group are sorted by their rendered input string.
#[must_use]
pub fn group_bindings(profile: &Profile) -> Vec<GroupedSubProfile<'_>> {
    profile
        .sub_profiles
        .iter()
        .map(|sp| {
            let mut bindings: Vec<GroupedBinding> = sp
                .bindings()
                .map(|b| GroupedBinding {
                    input: b
                        .input
                        .as_ref()
                        .map_or_else(|| "(none)".to_string(), Input::to_csv),
                    output: b.output.to_csv(),
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

    #[test]
    fn groups_single_subprofile_with_single_binding() {
        let p = profile_with_one_binding();
        let g = group_bindings(&p);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "Main");
        assert_eq!(g[0].bindings.len(), 1);
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
