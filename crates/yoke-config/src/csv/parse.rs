use std::io::Cursor;

use csv::{ReaderBuilder, Trim};

use crate::csv::raw::{RawCsv, RawRow, RawSection};
use crate::error::ParseError;

pub fn read_raw(input: &[u8]) -> Result<RawCsv, ParseError> {
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
    let top_line = first_chunk_rows.remove(0).cells;

    let mut sections: Vec<RawSection> = Vec::new();
    let mut blank_runs: Vec<usize> = Vec::new();

    if !first_chunk_rows.is_empty() {
        sections.push(RawSection {
            rows: first_chunk_rows,
        });
        blank_runs.push(chunks[0].trailing_blanks);
    }

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

    Ok(RawCsv {
        top_line,
        sections,
        blank_runs,
    })
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
        let r = rec.map_err(|e| ParseError::Csv(e.to_string()))?;
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
    SubProfileHeader, TopLine,
};

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub raw: RawCsv,
    pub model: Profile,
    pub warnings: Vec<Warning>,
}

pub fn parse(input: &[u8]) -> Result<ParseResult, ParseError> {
    let raw = read_raw(input)?;
    let mut warnings = Vec::new();
    let model = build_model(&raw, &mut warnings);
    Ok(ParseResult {
        raw,
        model,
        warnings,
    })
}

fn build_model(raw: &RawCsv, warnings: &mut Vec<Warning>) -> Profile {
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
                let (sp, ws) = build_sub_profile(section);
                sub_profiles.push(sp);
                warnings.extend(ws);
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
    }
}

fn build_sub_profile(section: &RawSection) -> (SubProfile, Vec<Warning>) {
    let mut warnings = Vec::new();
    let header = build_sub_profile_header(section);
    let body: Vec<&RawRow> = section.rows.iter().skip(3).collect();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut overrides: Vec<PreferenceOverride> = Vec::new();
    let mut seen_blank_output = false;

    for (idx, row) in body.iter().enumerate() {
        let output_cell = row.cells.first().map_or("", String::as_str);
        if output_cell.is_empty() {
            seen_blank_output = true;
            continue;
        }
        if seen_blank_output {
            warnings.push(Warning::DataAfterTerminator {
                line: idx,
                count: 1,
            });
        }
        let modifier_cell = row.cells.get(1).map_or("", String::as_str);
        let input_cell = row.cells.get(2).map_or("", String::as_str);
        let comment_cells: Vec<&str> = row
            .cells
            .iter()
            .skip(10)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .collect();
        let comment = if comment_cells.is_empty() {
            None
        } else {
            Some(comment_cells.join(" "))
        };

        if PreferenceSpec::for_id(output_cell).is_some() {
            let key = PreferenceKey::from_csv(output_cell).unwrap();
            overrides.push(PreferenceOverride {
                key,
                value: input_cell.to_owned(),
                comment,
            });
            continue;
        }

        let output = Output::from_csv(output_cell).unwrap();
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
            let i = Input::from_csv(input_cell).unwrap();
            if matches!(i, Input::Unknown(_)) {
                warnings.push(Warning::UnknownInput {
                    id: input_cell.into(),
                    line: idx,
                });
            }
            Some(i)
        };

        bindings.push(Binding {
            output,
            modifier,
            input,
            comment,
        });
    }

    (
        SubProfile {
            header,
            bindings,
            overrides,
        },
        warnings,
    )
}

fn build_sub_profile_header(section: &RawSection) -> SubProfileHeader {
    let cell = |row: usize, col: usize| -> String {
        section
            .rows
            .get(row)
            .and_then(|r| r.cells.get(col))
            .cloned()
            .unwrap_or_default()
    };

    let profile_name = cell(0, 1);
    let mode_raw = cell(0, 2);
    let mode = SubProfileMode::from_csv(&mode_raw)
        .unwrap_or_else(|| SubProfileMode::Unknown(mode_raw.clone()));
    let sub_mode = cell(1, 2);
    let channel_raw = cell(2, 2);
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

    let body: Vec<&RawRow> = section.rows.iter().skip(3).collect();
    for (idx, row) in body.iter().enumerate() {
        let id = row.cells.first().map_or("", String::as_str);
        if id.is_empty() {
            break;
        }
        let value = row.cells.get(1).cloned().unwrap_or_default();
        let units = row.cells.get(2).cloned().unwrap_or_default();
        let descr = row.cells.get(3).cloned().unwrap_or_default();
        let key = PreferenceKey::from_csv(id).unwrap();
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
                comment: None,
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
        assert_eq!(sp.bindings.len(), 2);
        assert!(result.warnings.is_empty());
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
        assert_eq!(sp.bindings.len(), 1, "only mouse_left is a binding");
        assert_eq!(sp.overrides.len(), 1);
        assert_eq!(
            sp.overrides[0].key,
            PreferenceKey::from_csv("joystick_dead_zone_shape").unwrap()
        );
        assert_eq!(sp.overrides[0].value, "1");
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
        assert_eq!(r.model.sub_profiles[0].bindings.len(), 1);
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
