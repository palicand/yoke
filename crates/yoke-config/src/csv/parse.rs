use std::io::Cursor;

use csv::{ReaderBuilder, Trim};

use crate::csv::raw::{RawCsv, RawRow, RawSection};
use crate::error::ParseError;

pub fn read_raw(input: &[u8]) -> Result<RawCsv, ParseError> {
    read_raw_meta(input).map(|(raw, _)| raw)
}

/// Parse to `RawCsv` and report whether the `QuadStick Configuration` header was
/// absent (a community Google-Sheet export, for which we synthesized a top
/// line). The flag gates horizontal multi-group sub-profile expansion, which
/// applies only to community sheets — device CSVs spread one sub-profile's
/// metadata (name, mode, channel) across adjacent columns and must stay
/// one-section-per-sub-profile.
fn read_raw_meta(input: &[u8]) -> Result<(RawCsv, bool), ParseError> {
    if std::str::from_utf8(input).is_err() {
        return Err(ParseError::Encoding);
    }

    // Split into runs of non-blank lines separated by blank-line runs.
    // We do this before feeding to the csv crate because csv::Reader silently
    // drops blank lines rather than surfacing them as records.
    let chunks = split_chunks(input);

    if chunks.is_empty() || chunks[0].lines.is_empty() {
        return Err(ParseError::MissingTopLine);
    }

    let mut first_chunk_rows = parse_chunk(&chunks[0].lines)?;
    if first_chunk_rows.is_empty() {
        return Err(ParseError::MissingTopLine);
    }
    // Device-saved CSVs lead with a `QuadStick Configuration` metadata row;
    // community Google-Sheet exports omit it and start at `Profile Name`. When
    // the header is absent, synthesize a default top line and keep every row as
    // profile body so the sub-profile parser still sees its `Profile Name`
    // section instead of consuming it as a (mismatched) top line.
    let has_header = first_chunk_rows[0]
        .cells
        .first()
        .is_some_and(|c| c.trim() == "QuadStick Configuration");
    let top_line = if has_header {
        first_chunk_rows.remove(0).cells
    } else {
        synthetic_top_line()
    };

    let mut sections: Vec<RawSection> = Vec::new();
    let mut blank_runs: Vec<usize> = Vec::new();

    // If the first chunk holds only the top line, its trailing blanks belong before
    // the first real section — capture them separately so they aren't dropped.
    let leading_blanks = if first_chunk_rows.is_empty() {
        chunks[0].trailing_blanks
    } else {
        sections.push(RawSection {
            rows: first_chunk_rows,
        });
        blank_runs.push(chunks[0].trailing_blanks);
        0
    };

    for chunk in &chunks[1..] {
        if chunk.lines.is_empty() {
            continue;
        }
        let rows = parse_chunk(&chunk.lines)?;
        if rows.is_empty() {
            continue;
        }
        sections.push(RawSection { rows });
        blank_runs.push(chunk.trailing_blanks);
    }

    // Pad blank_runs so it matches sections length (last section may have 0 trailing blanks).
    while blank_runs.len() < sections.len() {
        blank_runs.push(0);
    }

    Ok((
        RawCsv {
            top_line,
            leading_blanks,
            sections,
            blank_runs,
        },
        !has_header,
    ))
}

struct Chunk {
    /// Non-blank lines (raw bytes, including line endings) in this chunk.
    lines: Vec<Vec<u8>>,
    /// Number of blank lines that immediately follow this chunk.
    trailing_blanks: usize,
}

fn split_chunks(input: &[u8]) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut current_lines: Vec<Vec<u8>> = Vec::new();
    let mut blank_count: usize = 0;

    for line in lines_with_endings(input) {
        let content = strip_line_ending(line);
        if content.is_empty() {
            if !current_lines.is_empty() || !chunks.is_empty() {
                blank_count += 1;
            }
        } else if blank_count > 0 && !current_lines.is_empty() {
            // Blank run between non-blank content — flush the current chunk.
            chunks.push(Chunk {
                lines: std::mem::take(&mut current_lines),
                trailing_blanks: blank_count,
            });
            blank_count = 0;
            current_lines.push(line.to_owned());
        } else {
            blank_count = 0;
            current_lines.push(line.to_owned());
        }
    }

    if !current_lines.is_empty() {
        chunks.push(Chunk {
            lines: current_lines,
            trailing_blanks: blank_count,
        });
    }

    chunks
}

fn lines_with_endings(mut input: &[u8]) -> impl Iterator<Item = &[u8]> {
    std::iter::from_fn(move || {
        if input.is_empty() {
            return None;
        }
        let end = input
            .iter()
            .position(|&b| b == b'\n')
            .map_or(input.len(), |i| i + 1);
        let (line, rest) = input.split_at(end);
        input = rest;
        Some(line)
    })
}

fn strip_line_ending(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\r\n").unwrap_or(line);
    line.strip_suffix(b"\n").unwrap_or(line)
}

fn parse_chunk(lines: &[Vec<u8>]) -> Result<Vec<RawRow>, ParseError> {
    let mut buf: Vec<u8> = Vec::new();
    for line in lines {
        buf.extend_from_slice(line);
    }

    let mut rdr = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::None)
        .from_reader(Cursor::new(&buf));

    let mut rows: Vec<RawRow> = Vec::new();
    for rec in rdr.records() {
        let r = rec?;
        rows.push(RawRow {
            cells: r.iter().map(str::to_owned).collect(),
        });
    }
    Ok(rows)
}

use crate::catalog::{
    Channel, Input, Modifier, Output, PreferenceKey, PreferenceSpec, SubProfileMode,
};
use crate::error::Warning;
use crate::model::{
    Binding, PreferenceEntry, PreferenceOverride, Preferences, Profile, SubProfile,
    SubProfileHeader, SubProfileRow, TopLine,
};

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub raw: RawCsv,
    pub model: Profile,
    pub warnings: Vec<Warning>,
}

pub fn parse(input: &[u8]) -> Result<ParseResult, ParseError> {
    let (raw, community) = read_raw_meta(input)?;
    let mut warnings = Vec::new();
    let model = build_model(&raw, community, &mut warnings);
    Ok(ParseResult {
        raw,
        model,
        warnings,
    })
}

fn build_model(raw: &RawCsv, community: bool, warnings: &mut Vec<Warning>) -> Profile {
    let top_line = build_top_line(&raw.top_line);

    let mut sub_profiles: Vec<SubProfile> = Vec::new();
    let mut preferences: Option<Preferences> = None;
    let mut infrared: Vec<RawSection> = Vec::new();

    for section in &raw.sections {
        let section_type = section
            .rows
            .first()
            .and_then(|r| r.cells.first().map(String::as_str))
            .unwrap_or("");

        match section_type {
            "Profile Name" => {
                // Community Google-Sheet exports pack every sub-profile into a
                // single section as horizontal column groups (mode label at
                // cols 2, 10, ...); device CSVs use one vertical section per
                // sub-profile and spread that sub-profile's own name/mode/
                // channel across adjacent columns. So horizontal expansion is
                // gated to community sheets only — a device CSV's adjacent
                // metadata columns must never be read as extra sub-profiles.
                let groups = if community {
                    group_columns(section)
                } else {
                    Vec::new()
                };
                if groups.len() > 1 {
                    for col in groups {
                        let (sp, ws) = build_sub_profile(section, col, None);
                        sub_profiles.push(sp);
                        warnings.extend(ws);
                    }
                } else {
                    let (sp, ws) = build_sub_profile(section, 2, Some(10));
                    sub_profiles.push(sp);
                    warnings.extend(ws);
                }
            }
            "Preferences" => {
                let (p, ws) = build_preferences(section);
                preferences = Some(p);
                warnings.extend(ws);
            }
            // Infrared and any future unknown section types are forwarded opaquely
            // so round-trips don't lose data introduced by newer firmware versions.
            _ => {
                infrared.push(section.clone());
            }
        }
    }

    Profile {
        top_line,
        sub_profiles,
        preferences,
        infrared,
    }
}

// Default top line for community CSVs that ship without the device's
// `QuadStick Configuration` header. Mirrors the device header's 4-cell shape so
// a normalized profile writes back as a valid device file.
fn synthetic_top_line() -> Vec<String> {
    vec![
        "QuadStick Configuration".to_string(),
        "Version 1.4".to_string(),
        String::new(),
        String::new(),
    ]
}

fn build_top_line(cells: &[String]) -> TopLine {
    let get = |i: usize| cells.get(i).cloned().unwrap_or_default();
    let trailing = if cells.len() > 4 {
        cells[4..].to_vec()
    } else {
        Vec::new()
    };
    TopLine {
        label: get(0),
        version: get(1),
        source: get(2),
        title: get(3),
        trailing_cells: trailing,
        width: cells.len(),
    }
}

// Columns (>= 2) in the `Profile Name` row that carry a sub-profile label.
// One column is the vertical device layout; several mean a horizontal community
// sheet with one sub-profile group per labeled column.
fn group_columns(section: &RawSection) -> Vec<usize> {
    section.rows.first().map_or_else(Vec::new, |row| {
        row.cells
            .iter()
            .enumerate()
            .skip(2)
            .filter(|(_, c)| !c.trim().is_empty())
            .map(|(i, _)| i)
            .collect()
    })
}

// Builds one SubProfile from `section`, reading binding values from
// `value_col` (col 2 for vertical device CSVs; the group's column for a
// horizontal community sheet). `comment_start`, when set, folds trailing cells
// from that column onward into the binding comment — disabled for horizontal
// groups so the next group's column isn't mistaken for a comment.
fn build_sub_profile(
    section: &RawSection,
    value_col: usize,
    comment_start: Option<usize>,
) -> (SubProfile, Vec<Warning>) {
    let mut warnings = Vec::new();
    let header = build_sub_profile_header(section, value_col);
    let mut rows: Vec<SubProfileRow> = Vec::new();
    let mut seen_blank_output = false;

    for (idx, row) in section.rows.iter().skip(3).enumerate() {
        let output_cell = row.cells.first().map_or("", String::as_str);
        if output_cell.is_empty() {
            seen_blank_output = true;
            continue;
        }
        if seen_blank_output {
            warnings.push(Warning::DataAfterTerminator { line: idx });
        }
        let modifier_cell = row.cells.get(1).map_or("", String::as_str);
        let input_cell = row.cells.get(value_col).map_or("", String::as_str);
        let comment = comment_start.and_then(|start| {
            let mut c = String::new();
            for cell in row.cells.iter().skip(start).filter(|s| !s.is_empty()) {
                if !c.is_empty() {
                    c.push(' ');
                }
                c.push_str(cell);
            }
            (!c.is_empty()).then_some(c)
        });

        if PreferenceSpec::for_id(output_cell).is_some() {
            let key = PreferenceKey::from_csv(output_cell);
            rows.push(SubProfileRow::Override(PreferenceOverride {
                key,
                value: input_cell.to_owned(),
                comment,
            }));
            continue;
        }

        let output = Output::from_csv(output_cell);
        if matches!(output, Output::Unknown(_)) {
            warnings.push(Warning::UnknownOutput {
                id: output_cell.into(),
                line: idx,
            });
        }

        let modifier = Modifier::from_csv(modifier_cell).unwrap_or(Modifier::Normal);
        if matches!(modifier, Modifier::Unknown { .. }) {
            warnings.push(Warning::UnknownModifier {
                name: modifier_cell.split_whitespace().next().unwrap_or("").into(),
                line: idx,
            });
        }

        let input = if input_cell.is_empty() {
            None
        } else {
            let i = Input::from_csv(input_cell);
            if matches!(i, Input::Unknown(_)) {
                warnings.push(Warning::UnknownInput {
                    id: input_cell.into(),
                    line: idx,
                });
            }
            Some(i)
        };

        rows.push(SubProfileRow::Binding(Binding {
            output,
            modifier,
            input,
            comment,
        }));
    }

    (SubProfile { header, rows }, warnings)
}

fn build_sub_profile_header(section: &RawSection, value_col: usize) -> SubProfileHeader {
    let cell = |row: usize, col: usize| -> String {
        section
            .rows
            .get(row)
            .and_then(|r| r.cells.get(col))
            .cloned()
            .unwrap_or_default()
    };

    let profile_name = cell(0, 1);
    let mode_raw = cell(0, value_col);
    let mode = SubProfileMode::from_csv(&mode_raw)
        .unwrap_or_else(|| SubProfileMode::Unknown(mode_raw.clone()));
    let sub_mode = cell(1, value_col);
    let channel_raw = cell(2, value_col);
    let channel = Channel::from_csv(&channel_raw).unwrap_or(Channel::Usb);
    let column_header_label = cell(2, 0);

    SubProfileHeader {
        profile_name,
        mode,
        sub_mode,
        channel,
        column_header_label,
    }
}

fn build_preferences(section: &RawSection) -> (Preferences, Vec<Warning>) {
    let mut warnings = Vec::new();
    let mut entries: Vec<(String, PreferenceEntry)> = Vec::new();

    for (idx, row) in section.rows.iter().skip(3).enumerate() {
        let id = row.cells.first().map_or("", String::as_str);
        if id.is_empty() {
            break;
        }
        let value = row.cells.get(1).cloned().unwrap_or_default();
        let units = row.cells.get(2).cloned().unwrap_or_default();
        let descr = row.cells.get(3).cloned().unwrap_or_default();
        let comment = row
            .cells
            .get(4)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let key = PreferenceKey::from_csv(id);
        if matches!(key, PreferenceKey::Unknown(_)) {
            warnings.push(Warning::UnknownPreference {
                id: id.into(),
                line: idx,
            });
        } else if let Some(spec) = PreferenceSpec::for_id(id)
            && let Err(reason) = spec.validate(&value)
        {
            warnings.push(Warning::PreferenceOutOfRange {
                key: id.into(),
                value: value.clone(),
                expected: reason,
                line: idx,
            });
        }
        entries.push((
            id.to_owned(),
            PreferenceEntry {
                key,
                value,
                units,
                description: descr,
                comment,
            },
        ));
    }

    (Preferences { entries }, warnings)
}

#[cfg(test)]
mod parse_tests {
    use super::*;
    use crate::catalog::{Channel, PreferenceKey, SubProfileMode};

    const SINGLE_SUB: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n";

    #[test]
    fn parses_single_sub_profile() {
        let result = parse(SINGLE_SUB).expect("parse");
        assert_eq!(result.model.sub_profiles.len(), 1);
        let sp = &result.model.sub_profiles[0];
        assert_eq!(sp.header.mode, SubProfileMode::Mouse);
        assert_eq!(sp.header.channel, Channel::Usb);
        assert_eq!(sp.bindings().count(), 2);
        assert!(result.warnings.is_empty());
    }

    // Community Google-Sheet export: same body as SINGLE_SUB but without the
    // leading `QuadStick Configuration` row.
    const HEADERLESS_COMMUNITY: &[u8] = b"Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n";

    #[test]
    fn headerless_community_csv_synthesizes_top_line_and_parses() {
        let result = parse(HEADERLESS_COMMUNITY).expect("headerless CSV must parse");
        // Header is synthesized rather than consuming the `Profile Name` row.
        assert_eq!(result.model.top_line.label, "QuadStick Configuration");
        assert_eq!(result.model.sub_profiles.len(), 1);
        assert_eq!(result.model.sub_profiles[0].bindings().count(), 2);
    }

    // Two sub-profiles laid out as horizontal column groups (cols 2 and 10),
    // as community Google-Sheet exports do. Each group's value lives in its own
    // column; the modifier (col 1) is shared.
    const HORIZONTAL_COMMUNITY: &[u8] = b"Profile Name,,Left joy,,,,,,,,Mixed joy\r\n\
prof.csv,,Normal,,,,,,,,Alternate\r\n\
Output or Function,Function,usb,,,,,,,,usb\r\n\
left_joy_left,normal,left,,,,,,,,\r\n\
right_joy_left,normal,,,,,,,,,left\r\n\
\r\n";

    #[test]
    fn horizontal_community_csv_expands_each_group_into_a_sub_profile() {
        let result = parse(HORIZONTAL_COMMUNITY).expect("horizontal CSV must parse");
        assert_eq!(result.model.sub_profiles.len(), 2);
        // Each group reads bindings from its own column, so exactly one binding
        // per group carries an input.
        let bound = |sp: &SubProfile| sp.bindings().filter(|b| b.input.is_some()).count();
        assert_eq!(bound(&result.model.sub_profiles[0]), 1);
        assert_eq!(bound(&result.model.sub_profiles[1]), 1);
    }

    // A device CSV carries the sub-profile name in col 1 and channel in col 3
    // of the `Profile Name` row, alongside the mode in col 2. Those adjacent
    // metadata columns must not be mistaken for horizontal community groups:
    // a device CSV is always one sub-profile per section.
    const DEVICE_NAMED_SUB: &[u8] = b"QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

    #[test]
    fn device_csv_with_named_subprofile_is_single_not_horizontal() {
        let r = parse(DEVICE_NAMED_SUB).expect("parse");
        assert_eq!(r.model.sub_profiles.len(), 1);
        let sp = &r.model.sub_profiles[0];
        assert_eq!(sp.header.profile_name, "Main");
        assert_eq!(sp.header.mode, SubProfileMode::Mouse);
        assert_eq!(sp.header.channel, Channel::Usb);
        assert_eq!(sp.bindings().count(), 1);
    }

    const WITH_PREFS_OVERRIDE: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
PlayStation Outputs,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
joystick_dead_zone_shape,normal,1,\r\n\
\r\n";

    #[test]
    fn preference_row_parses_as_override_not_binding() {
        let r = parse(WITH_PREFS_OVERRIDE).expect("parse");
        let sp = &r.model.sub_profiles[0];
        assert_eq!(sp.bindings().count(), 1, "only mouse_left is a binding");
        let overrides: Vec<_> = sp.overrides().collect();
        assert_eq!(overrides.len(), 1);
        assert_eq!(
            overrides[0].key,
            PreferenceKey::from_csv("joystick_dead_zone_shape")
        );
        assert_eq!(overrides[0].value, "1");
    }

    const WITH_PREFS_SECTION: &[u8] = b"QuadStick Configuration,Version 1.1,abc,Mac\r\n\
Preferences,\r\n\
prefs.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,40,,,\r\n\
brightness,75,,,\r\n\
\r\n";

    #[test]
    fn parses_preferences_section() {
        let r = parse(WITH_PREFS_SECTION).expect("parse");
        let prefs = r.model.preferences.expect("must have prefs");
        assert_eq!(prefs.entries.len(), 2);
        let vol = prefs
            .entries
            .iter()
            .find(|(k, _)| k == "volume")
            .map(|(_, e)| &*e.value);
        assert_eq!(vol, Some("40"));
    }

    const WITH_UNKNOWN: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mystery_output,future_modifier,unknown_input_id,\r\n\
\r\n";

    #[test]
    fn unknown_vocabulary_emits_warnings_but_loads() {
        let r = parse(WITH_UNKNOWN).expect("parse");
        assert_eq!(r.model.sub_profiles[0].bindings().count(), 1);
        assert!(
            r.warnings
                .iter()
                .any(|w| matches!(w, crate::Warning::UnknownOutput { .. }))
        );
        assert!(
            r.warnings
                .iter()
                .any(|w| matches!(w, crate::Warning::UnknownModifier { .. }))
        );
        assert!(r.warnings.iter().any(|w| matches!(w,
            crate::Warning::UnknownInput { id, .. } if id == "unknown_input_id")));
    }

    const WITH_MODE_SYNONYM: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Left joy,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
left_joy_up,normal,up,\r\n\
\r\n";

    #[test]
    fn legacy_mode_synonym_parses_to_canonical() {
        let r = parse(WITH_MODE_SYNONYM).expect("parse");
        assert_eq!(
            r.model.sub_profiles[0].header.mode,
            SubProfileMode::LeftAnalog
        );
    }

    const WITH_BLUETOOTH: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,Bluetooth,\r\n\
mouse_left,normal,left,\r\n\
\r\n";

    #[test]
    fn bluetooth_channel_recognised() {
        let r = parse(WITH_BLUETOOTH).expect("parse");
        assert_eq!(r.model.sub_profiles[0].header.channel, Channel::Bluetooth);
    }
}
