use crate::csv::raw::RawCsv;

pub fn write_raw(raw: &RawCsv) -> Vec<u8> {
    let mut out = String::new();
    push_row(&mut out, &raw.top_line);
    for _ in 0..raw.leading_blanks {
        out.push_str("\r\n");
    }
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

    if sp_idx < profile.sub_profiles.len() {
        return Err(WriteError::InvariantViolation(format!(
            "model has {} sub-profiles but template has only {sp_idx} sub-profile section(s)",
            profile.sub_profiles.len()
        )));
    }

    if !prefs_done && let Some(prefs) = &profile.preferences {
        sections.push(build_prefs_section(prefs));
    }

    let raw = RawCsv {
        top_line: top_line_to_cells(profile),
        leading_blanks: template.leading_blanks,
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
        leading_blanks: 0,
        sections,
        blank_runs,
    };
    write_raw(&raw)
}

// The template path must be byte-faithful: a model that came straight out of the
// parser has to write back to the exact input bytes. Rebuilding a row from the
// model alone cannot do that, because the model is a lossy view of the row --
// cells past the modelled columns (and the source's own column count) exist only
// in the template. So each row is emitted from its template row with just the
// cells whose model value actually changed overwritten.
fn rebuild_sub_profile(sp: &SubProfile, template: &RawSection) -> RawSection {
    let mut header_rows: Vec<RawRow> = template.rows.iter().take(3).cloned().collect();
    // Persist the model's profile name into the verbatim-copied header. The name is the
    // only header field a same-section-count edit (RenameSubProfile) can change, and the
    // parser reads it untransformed from cell (0,1), so writing it back is byte-identical
    // when unchanged and correct when renamed. Mode/sub-mode/channel are deliberately left
    // verbatim: their canonical_csv form is not guaranteed to match the stored cell, and no
    // current op mutates them without also changing the section count (which routes to the
    // canonical writer instead).
    if let Some(first) = header_rows.first_mut() {
        if first.cells.len() < 2 {
            first.cells.resize(2, String::new());
        }
        first.cells[1].clone_from(&sp.header.profile_name);
    }
    let data = template.rows.get(3..).unwrap_or_default();
    let fresh_width = data.first().map_or_else(
        || template.rows.first().map_or(4, |r| r.cells.len()),
        |r| r.cells.len(),
    );

    let mut rows = header_rows;
    let mut cursor = 0usize;
    for trow in data {
        let key = trow.cells.first().map_or("", String::as_str);
        // The parser skips rows with a blank output cell, so they have no model
        // counterpart and are carried through in place.
        if key.is_empty() {
            rows.push(trow.clone());
            continue;
        }
        let Some(matched) =
            (cursor..sp.rows.len()).find(|&i| sub_profile_row_key(&sp.rows[i]) == key)
        else {
            // The model no longer carries this row: the edit deleted it.
            continue;
        };
        for row in &sp.rows[cursor..matched] {
            rows.push(fresh_sub_profile_row(row, fresh_width));
        }
        rows.push(merge_sub_profile_row(&sp.rows[matched], trow));
        cursor = matched + 1;
    }
    for row in &sp.rows[cursor..] {
        rows.push(fresh_sub_profile_row(row, fresh_width));
    }
    RawSection { rows }
}

fn sub_profile_row_key(row: &SubProfileRow) -> String {
    match row {
        SubProfileRow::Binding(b) => b.output.to_csv(),
        SubProfileRow::Override(o) => o.key.as_csv(),
    }
}

fn fresh_sub_profile_row(row: &SubProfileRow, width: usize) -> RawRow {
    match row {
        SubProfileRow::Binding(b) => binding_row(b, width),
        SubProfileRow::Override(o) => override_row(o, width),
    }
}

fn merge_sub_profile_row(row: &SubProfileRow, template: &RawRow) -> RawRow {
    let mut cells = template.cells.clone();
    match row {
        SubProfileRow::Binding(b) => {
            set_cell(&mut cells, 0, &b.output.to_csv());
            set_cell(&mut cells, 1, &b.modifier.to_csv());
            set_cell(
                &mut cells,
                VALUE_COL,
                &b.input
                    .as_ref()
                    .map(crate::catalog::Input::to_csv)
                    .unwrap_or_default(),
            );
            merge_comment(&mut cells, b.comment.as_deref());
        }
        SubProfileRow::Override(o) => {
            set_cell(&mut cells, 0, &o.key.as_csv());
            // Column 1 is left verbatim: the parser does not read a modifier for
            // an override row, so the model has no value to write back.
            set_cell(&mut cells, VALUE_COL, &o.value);
            merge_comment(&mut cells, o.comment.as_deref());
        }
    }
    RawRow { cells }
}

// Writes `value` only when it differs from what the parser read out of this cell,
// so an unchanged field never widens the row or rewrites the source's own spelling.
// A missing cell reads as empty, matching the parser.
fn set_cell(cells: &mut Vec<String>, idx: usize, value: &str) {
    if cells.get(idx).map_or("", String::as_str) == value {
        return;
    }
    if cells.len() <= idx {
        cells.resize(idx + 1, String::new());
    }
    value.clone_into(&mut cells[idx]);
}

// Mirrors the parser's comment fold: every non-empty cell from COMMENT_COL on,
// joined by a space.
fn template_comment(cells: &[String]) -> Option<String> {
    let mut c = String::new();
    for cell in cells.iter().skip(COMMENT_COL).filter(|s| !s.is_empty()) {
        if !c.is_empty() {
            c.push(' ');
        }
        c.push_str(cell);
    }
    (!c.is_empty()).then_some(c)
}

// The comment spans every cell from COMMENT_COL on, so a changed comment has to
// replace the whole run rather than just its first cell.
fn merge_comment(cells: &mut Vec<String>, comment: Option<&str>) {
    if template_comment(cells).as_deref() == comment {
        return;
    }
    cells.truncate(COMMENT_COL.min(cells.len()));
    if let Some(c) = comment {
        place_comment(cells, c);
    }
}

const COMMENT_COL: usize = 10;
// The column a vertical device CSV keeps binding inputs and override values in.
const VALUE_COL: usize = 2;

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
                entry.comment.clone().unwrap_or_default(),
            ],
        });
    }
    RawSection { rows }
}

fn rebuild_preferences(profile: &Profile, template: &RawSection) -> RawSection {
    let header_rows: Vec<RawRow> = template.rows.iter().take(3).cloned().collect();
    let Some(prefs) = &profile.preferences else {
        return RawSection { rows: header_rows };
    };
    let data = template.rows.get(3..).unwrap_or_default();
    // Width for rows the model adds; existing rows keep their own.
    let fresh_width = data.first().map_or(5, |r| r.cells.len()).max(5);

    let mut rows = header_rows;
    let mut cursor = 0usize;
    let mut trailing: &[RawRow] = &[];
    for (i, trow) in data.iter().enumerate() {
        let key = trow.cells.first().map_or("", String::as_str);
        // The parser stops at the first blank id, so nothing from here on is
        // modelled and all of it has to survive verbatim.
        if key.is_empty() {
            trailing = &data[i..];
            break;
        }
        let Some(matched) = (cursor..prefs.entries.len()).find(|&i| prefs.entries[i].0 == key)
        else {
            continue;
        };
        for (id, entry) in &prefs.entries[cursor..matched] {
            rows.push(fresh_preference_row(id, entry, fresh_width));
        }
        let (id, entry) = &prefs.entries[matched];
        rows.push(merge_preference_row(id, entry, trow));
        cursor = matched + 1;
    }
    for (id, entry) in &prefs.entries[cursor..] {
        rows.push(fresh_preference_row(id, entry, fresh_width));
    }
    rows.extend(trailing.iter().cloned());
    RawSection { rows }
}

fn merge_preference_row(
    id: &str,
    entry: &crate::model::PreferenceEntry,
    template: &RawRow,
) -> RawRow {
    let mut cells = template.cells.clone();
    set_cell(&mut cells, 0, id);
    set_cell(&mut cells, 1, &entry.value);
    set_cell(&mut cells, 2, &entry.units);
    set_cell(&mut cells, 3, &entry.description);
    // The parser trims the comment cell, so compare trimmed before deciding the
    // model changed it.
    let existing = cells.get(4).map(|s| s.trim()).filter(|s| !s.is_empty());
    if existing != entry.comment.as_deref() {
        set_cell(&mut cells, 4, entry.comment.as_deref().unwrap_or(""));
    }
    RawRow { cells }
}

fn fresh_preference_row(id: &str, entry: &crate::model::PreferenceEntry, width: usize) -> RawRow {
    let mut cells = vec![
        id.to_owned(),
        entry.value.clone(),
        entry.units.clone(),
        entry.description.clone(),
    ];
    pad_to(&mut cells, width);
    if let Some(c) = &entry.comment {
        if cells.len() > 4 {
            cells[4].clone_from(c);
        } else {
            cells.resize(4, String::new());
            cells.push(c.clone());
        }
    }
    RawRow { cells }
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
    fn renamed_sub_profile_persists_through_template_path() {
        // A rename keeps the section count, so it stays on the template-fidelity writer;
        // the new name must still survive the write rather than being silently dropped.
        let r = parse(FIXTURE).expect("parse");
        let mut model = r.model.clone();
        "Renamed".clone_into(&mut model.sub_profiles[0].header.profile_name);
        let bytes = write(&model, Some(&r.raw)).expect("write");
        let back = parse(&bytes).expect("parse back");
        assert_eq!(back.model.sub_profiles[0].header.profile_name, "Renamed");
    }

    #[test]
    fn write_canonical_parses_back_equivalently() {
        let r1 = parse(FIXTURE).expect("parse");
        let bytes = write(&r1.model, None).expect("write");
        let r2 = parse(&bytes).expect("parse 2");
        assert_eq!(r1.model, r2.model, "model survives canonical round-trip");
    }

    #[test]
    fn write_with_template_errors_if_model_has_extra_sub_profiles() {
        let r = parse(FIXTURE).expect("parse");
        let mut model = r.model.clone();
        let extra = model.sub_profiles[0].clone();
        model.sub_profiles.push(extra);
        match write(&model, Some(&r.raw)) {
            Err(WriteError::InvariantViolation(msg)) => {
                assert!(msg.contains("sub-profiles"), "msg was: {msg}");
            }
            other => panic!("expected InvariantViolation, got {other:?}"),
        }
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

    #[test]
    fn rebuild_preferences_drops_data_rows_when_model_has_none() {
        const PREFS: &[u8] = b"QuadStick Configuration,Version 1.1,abc,Mac\r\n\
Preferences,\r\n\
prefs.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,40,,,\r\n\
brightness,75,,,\r\n\
\r\n";
        let r = parse(PREFS).expect("parse");
        let mut model = r.model.clone();
        model.preferences = None;
        let bytes = write(&model, Some(&r.raw)).expect("write");
        let out = std::str::from_utf8(&bytes).unwrap();
        assert!(out.contains("Preferences,"), "header retained: {out}");
        assert!(!out.contains("volume"), "stale row leaked: {out}");
        assert!(!out.contains("brightness"), "stale row leaked: {out}");
    }

    #[test]
    fn preference_comment_round_trips() {
        const WITH_COMMENT: &[u8] = b"QuadStick Configuration,Version 1.1,abc,Mac\r\n\
Preferences,\r\n\
prefs.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,40,,,note-here\r\n\
\r\n";
        let r = parse(WITH_COMMENT).expect("parse");
        let prefs = r.model.preferences.as_ref().expect("prefs");
        assert_eq!(prefs.entries[0].1.comment.as_deref(), Some("note-here"));
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(WITH_COMMENT).unwrap()
        );
    }

    // An unescaped comma in a preference description splits into extra cells that the
    // model does not carry. They live in the template and must survive the write.
    const SPLIT_DESCRIPTION: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Preferences,\r\n\
prefs.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
enable_DS3_emulation,3,,0=Normal composite device mode, 1=DS3 emulation, 2=X360CE mode,\r\n\
volume,40,,,\r\n\
\r\n";

    #[test]
    fn preference_row_with_split_description_round_trips() {
        let r = parse(SPLIT_DESCRIPTION).expect("parse");
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(SPLIT_DESCRIPTION).unwrap()
        );
    }

    #[test]
    fn editing_a_preference_value_keeps_the_rest_of_its_row() {
        let r = parse(SPLIT_DESCRIPTION).expect("parse");
        let mut model = r.model.clone();
        let prefs = model.preferences.as_mut().expect("prefs");
        "5".clone_into(&mut prefs.entries[0].1.value);
        let bytes = write(&model, Some(&r.raw)).expect("write");
        let out = std::str::from_utf8(&bytes).unwrap();
        assert!(
            out.contains(
                "enable_DS3_emulation,5,,0=Normal composite device mode, 1=DS3 emulation, 2=X360CE mode,\r\n"
            ),
            "row was: {out}"
        );
    }

    const MIXED_WIDTHS: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_wheel_up,normal,\r\n\
mouse_left,normal,left,\r\n\
,,,\r\n\
\r\n";

    #[test]
    fn rows_keep_their_own_column_count() {
        // A 3-cell binding row must not be padded out to the header's width, and the
        // all-empty row the parser skips has to stay where it was.
        let r = parse(MIXED_WIDTHS).expect("parse");
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(MIXED_WIDTHS).unwrap()
        );
    }

    #[test]
    fn deleting_a_binding_drops_only_its_row() {
        let r = parse(MIXED_WIDTHS).expect("parse");
        let mut model = r.model.clone();
        model.sub_profiles[0].rows.remove(0);
        let bytes = write(&model, Some(&r.raw)).expect("write");
        let out = std::str::from_utf8(&bytes).unwrap();
        assert!(!out.contains("mouse_wheel_up"), "stale row leaked: {out}");
        assert!(out.contains("mouse_left,normal,left,\r\n"), "out: {out}");
    }

    #[test]
    fn multi_cell_binding_comment_round_trips() {
        // The parser folds every cell from column 10 on into one comment; writing it
        // back as a single cell would drop the split.
        const WIDE_COMMENT: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,,,,,,,,note1,note2\r\n\
\r\n";
        let r = parse(WIDE_COMMENT).expect("parse");
        assert_eq!(
            r.model.sub_profiles[0].bindings().next().unwrap().comment,
            Some("note1 note2".to_owned())
        );
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(WIDE_COMMENT).unwrap()
        );
    }

    #[test]
    fn preferences_width_uses_max_data_row() {
        const WIDE: &[u8] = b"QuadStick Configuration,Version 1.1,abc,Mac\r\n\
Preferences,\r\n\
prefs.csv,,,,,\r\n\
Preference,Value,Units,Description,,\r\n\
volume,40,,,,\r\n\
\r\n";
        let r = parse(WIDE).expect("parse");
        let bytes = write(&r.model, Some(&r.raw)).expect("write");
        pretty_assertions::assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            std::str::from_utf8(WIDE).unwrap()
        );
    }
}
