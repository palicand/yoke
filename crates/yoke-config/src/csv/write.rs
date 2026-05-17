use crate::csv::raw::RawCsv;

pub fn write_raw(raw: &RawCsv) -> Vec<u8> {
    let mut out = String::new();
    push_row(&mut out, &raw.top_line);
    for (i, section) in raw.sections.iter().enumerate() {
        for row in &section.rows {
            push_row(&mut out, &row.cells);
        }
        let blanks = raw.blank_runs.get(i).copied().unwrap_or(0);
        for _ in 0..blanks {
            out.push_str("\r\n");
        }
    }
    out.into_bytes()
}

fn push_row(out: &mut String, cells: &[String]) {
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_cell(out, cell);
    }
    out.push_str("\r\n");
}

fn push_cell(out: &mut String, cell: &str) {
    if cell.contains(',') || cell.contains('"') || cell.contains('\n') || cell.contains('\r') {
        out.push('"');
        for ch in cell.chars() {
            if ch == '"' {
                out.push('"');
            }
            out.push(ch);
        }
        out.push('"');
    } else {
        out.push_str(cell);
    }
}

use crate::csv::raw::{RawRow, RawSection};
use crate::error::WriteError;
use crate::model::{Binding, PreferenceOverride, Profile, SubProfile, SubProfileRow};

pub fn write(profile: &Profile, template: Option<&RawCsv>) -> Result<Vec<u8>, WriteError> {
    template.map_or_else(
        || Ok(write_canonical(profile)),
        |t| write_with_template(profile, t),
    )
}

fn write_with_template(profile: &Profile, template: &RawCsv) -> Result<Vec<u8>, WriteError> {
    let mut sections: Vec<RawSection> = Vec::new();
    let blank_runs = template.blank_runs.clone();

    let mut sp_idx = 0usize;
    let mut prefs_done = false;
    let mut ir_idx = 0usize;

    for tsection in &template.sections {
        let section_type = tsection
            .rows
            .first()
            .and_then(|r| r.cells.first().map(String::as_str))
            .unwrap_or("");
        let rebuilt = match section_type {
            "Profile Name" => {
                let sp = profile.sub_profiles.get(sp_idx).ok_or_else(|| {
                    WriteError::InvariantViolation(format!(
                        "template has sub-profile section #{sp_idx} but model has only {} sub-profiles",
                        profile.sub_profiles.len()))
                })?;
                sp_idx += 1;
                rebuild_sub_profile(sp, tsection)
            }
            "Preferences" => {
                prefs_done = true;
                rebuild_preferences(profile, tsection)
            }
            _ => {
                let sec = profile
                    .infrared
                    .get(ir_idx)
                    .cloned()
                    .unwrap_or_else(|| tsection.clone());
                ir_idx += 1;
                sec
            }
        };
        sections.push(rebuilt);
    }

    if !prefs_done && let Some(prefs) = &profile.preferences {
        sections.push(build_prefs_section(prefs));
    }

    let raw = RawCsv {
        top_line: top_line_to_cells(profile),
        sections,
        blank_runs,
    };
    Ok(write_raw(&raw))
}

fn write_canonical(profile: &Profile) -> Vec<u8> {
    let mut sections: Vec<RawSection> = Vec::new();
    for sp in &profile.sub_profiles {
        sections.push(build_sub_profile_section(sp));
    }
    if let Some(prefs) = &profile.preferences {
        sections.push(build_prefs_section(prefs));
    }
    sections.extend(profile.infrared.iter().cloned());
    let blank_runs = vec![1usize; sections.len()];
    let raw = RawCsv {
        top_line: top_line_to_cells(profile),
        sections,
        blank_runs,
    };
    write_raw(&raw)
}

fn rebuild_sub_profile(sp: &SubProfile, template: &RawSection) -> RawSection {
    let header_rows: Vec<RawRow> = template.rows.iter().take(3).cloned().collect();
    let body_template_width = template.rows.first().map_or(4, |r| r.cells.len());

    let mut rows = header_rows;
    for row in &sp.rows {
        rows.push(match row {
            SubProfileRow::Binding(b) => binding_row(b, body_template_width),
            SubProfileRow::Override(o) => override_row(o, body_template_width),
        });
    }
    RawSection { rows }
}

const COMMENT_COL: usize = 10;

// Place a column-K (index 10) comment, padding earlier cells as needed.
fn place_comment(cells: &mut Vec<String>, comment: &str) {
    if cells.len() > COMMENT_COL {
        comment.clone_into(&mut cells[COMMENT_COL]);
    } else {
        cells.resize(COMMENT_COL, String::new());
        cells.push(comment.to_owned());
    }
}

fn binding_row(b: &Binding, width: usize) -> RawRow {
    let row_width = width.max(b.comment.as_ref().map_or(3, |_| COMMENT_COL + 1));
    let mut cells: Vec<String> = Vec::with_capacity(row_width);
    cells.push(b.output.to_csv());
    cells.push(b.modifier.to_csv());
    cells.push(
        b.input
            .as_ref()
            .map(crate::catalog::Input::to_csv)
            .unwrap_or_default(),
    );
    pad_to(&mut cells, width);
    if let Some(c) = &b.comment {
        place_comment(&mut cells, c);
    }
    RawRow { cells }
}

fn override_row(o: &PreferenceOverride, width: usize) -> RawRow {
    let row_width = width.max(o.comment.as_ref().map_or(3, |_| COMMENT_COL + 1));
    let mut cells: Vec<String> = Vec::with_capacity(row_width);
    cells.push(o.key.as_csv());
    cells.push("normal".to_owned());
    cells.push(o.value.clone());
    pad_to(&mut cells, width);
    if let Some(c) = &o.comment {
        place_comment(&mut cells, c);
    }
    RawRow { cells }
}

fn build_sub_profile_section(sp: &SubProfile) -> RawSection {
    let header_label = if sp.header.column_header_label.is_empty() {
        "Output or Function".to_owned()
    } else {
        sp.header.column_header_label.clone()
    };
    let mut rows = vec![
        RawRow {
            cells: vec![
                "Profile Name".into(),
                sp.header.profile_name.clone(),
                sp.header.mode.canonical_csv(),
                String::new(),
            ],
        },
        RawRow {
            cells: vec![
                String::new(),
                String::new(),
                sp.header.sub_mode.clone(),
                String::new(),
            ],
        },
        RawRow {
            cells: vec![
                header_label,
                "Function".into(),
                sp.header.channel.canonical_csv().to_owned(),
                String::new(),
            ],
        },
    ];
    for row in &sp.rows {
        rows.push(match row {
            SubProfileRow::Binding(b) => binding_row(b, 4),
            SubProfileRow::Override(o) => override_row(o, 4),
        });
    }
    RawSection { rows }
}

fn build_prefs_section(prefs: &crate::model::Preferences) -> RawSection {
    let mut rows = vec![
        RawRow {
            cells: vec!["Preferences".into()],
        },
        RawRow {
            cells: vec![String::new(); 5],
        },
        RawRow {
            cells: vec![
                "Preference".into(),
                "Value".into(),
                "Units".into(),
                "Description".into(),
                String::new(),
            ],
        },
    ];
    for (id, entry) in &prefs.entries {
        rows.push(RawRow {
            cells: vec![
                id.clone(),
                entry.value.clone(),
                entry.units.clone(),
                entry.description.clone(),
                String::new(),
            ],
        });
    }
    RawSection { rows }
}

fn rebuild_preferences(profile: &Profile, template: &RawSection) -> RawSection {
    let Some(prefs) = &profile.preferences else {
        return template.clone();
    };
    let mut rows: Vec<RawRow> = template.rows.iter().take(3).cloned().collect();
    let width = template.rows.first().map_or(5, |r| r.cells.len());
    for (id, entry) in &prefs.entries {
        let mut cells = vec![
            id.clone(),
            entry.value.clone(),
            entry.units.clone(),
            entry.description.clone(),
        ];
        pad_to(&mut cells, width);
        rows.push(RawRow { cells });
    }
    RawSection { rows }
}

fn top_line_to_cells(profile: &Profile) -> Vec<String> {
    let mut cells = vec![
        profile.top_line.label.clone(),
        profile.top_line.version.clone(),
        profile.top_line.source.clone(),
        profile.top_line.title.clone(),
    ];
    cells.extend(profile.top_line.trailing_cells.iter().cloned());
    // Preserve the source's column count: a 2-cell top line stays 2 cells,
    // a 4-cell stays 4, a 5-cell with trailing empties stays 5.
    if profile.top_line.width == 0 {
        // Default to 2 — the minimum that survives a parse round-trip.
        while cells.last().is_some_and(String::is_empty) && cells.len() > 2 {
            cells.pop();
        }
    } else {
        cells.truncate(profile.top_line.width);
        cells.resize(profile.top_line.width, String::new());
    }
    cells
}

fn pad_to(cells: &mut Vec<String>, target_width: usize) {
    if cells.len() < target_width {
        cells.resize(target_width, String::new());
    }
}

#[cfg(test)]
mod write_tests {
    use super::*;
    use crate::parse;

    const FIXTURE: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n";

    #[test]
    fn write_with_template_round_trips_bytes() {
        let r = parse(FIXTURE).expect("parse");
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(FIXTURE).unwrap()
        );
    }

    #[test]
    fn write_canonical_parses_back_equivalently() {
        let r1 = parse(FIXTURE).expect("parse");
        let bytes = write(&r1.model, None).expect("write");
        let r2 = parse(&bytes).expect("parse 2");
        assert_eq!(r1.model, r2.model, "model survives canonical round-trip");
    }

    #[test]
    fn preference_override_round_trips() {
        let input: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
joystick_dead_zone_shape,normal,1,\r\n\
\r\n";
        let r = parse(input).expect("parse");
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(input).unwrap()
        );
    }
}
