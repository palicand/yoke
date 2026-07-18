use crate::app::{PickerTarget, YokeApp};
use crate::stations::{station_by_id, station_inputs};
use crate::theme::{empty_output_button, mod_pill_style, output_button, output_color, row_frame};

// Row tuple for the unfiltered browse view: input label, output label,
// output glyph, output color, modifier label.
type Row = (String, String, String, egui::Color32, String);

// (input_csv, chord_rows): each chord row is
// (modifier_csv, output_csv, output_glyph, color).
type RosterEntry = (String, Vec<(String, String, String, egui::Color32)>);

// Deferred action collected in the roster loop, dispatched after borrows end.
enum RosterAction {
    EditOutput {
        input: String,
        modifier: String,
    },
    EditModifier {
        input: String,
        output: String,
        modifier: String,
    },
    ClearOne {
        input: String,
        modifier: String,
    },
    ClearAll {
        input: String,
    },
    Add {
        input: String,
    },
}

// The `.expect` calls in dispatch_action are guarded by the fact that `open_profile` is
// Some before the station-filtered branch is entered; they cannot fire in correct usage.
#[allow(clippy::missing_panics_doc)]
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let palette = *app.palette();
    let filter = app.selected_station();
    let sub_idx = app.selected_subprofile();

    if let Some(station) = filter {
        show_station(app, ui, &palette, sub_idx, station);
    } else {
        show_all(app, ui, &palette, sub_idx);
    }
}

/// Title-block data for the bindings pane header, derived from the selected
/// sub-profile's real mode and channel.
struct PaneTitle {
    title: String,
    subtitle: String,
}

/// Build the all-list pane title from the sub-profile's mode: serif
/// "<mode> bindings", subtitle "<mode> mode · <channel> output".
fn all_pane_title(sub: &yoke_config::model::SubProfile) -> PaneTitle {
    let mode = sub.header.mode.canonical_csv();
    let mode = if mode.trim().is_empty() {
        "Sub-profile".to_owned()
    } else {
        mode
    };
    let channel = sub.header.channel.canonical_csv();
    PaneTitle {
        title: format!("{mode} bindings"),
        subtitle: format!("{mode} mode · {channel} output"),
    }
}

/// Render the pane header: serif title, subtitle, and a right-aligned
/// display-only "Test" button plus an optional "Show all" filter-clear.
///
/// The "Test" button implies live on-device testing, which Yoke cannot do yet,
/// so it is rendered disabled (display-only) and wired to nothing.
fn show_pane_header(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    title: &PaneTitle,
    count: usize,
    can_clear_filter: bool,
) -> bool {
    let mut clear_filter = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.heading(&title.title);
            ui.add(egui::Label::new(
                egui::RichText::new(format!("{} · {count} bindings", title.subtitle))
                    .color(palette.ink_2)
                    .size(12.0),
            ));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            // Display-only: live device testing is not implemented. Disabled so
            // it never looks actionable; carries no behavior.
            ui.add_enabled(false, egui::Button::new("Test"))
                .on_disabled_hover_text("Live device testing is not available yet");
            if can_clear_filter && ui.small_button("Show all").clicked() {
                clear_filter = true;
            }
        });
    });
    clear_filter
}

/// Station view: full input roster with edit affordances.
fn show_station(
    app: &mut YokeApp,
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    sub_idx: usize,
    station: &'static str,
) {
    // Collect all display data while the immutable `app` borrow is live.
    let (total_rows, title, roster) = {
        let Some(open) = app.open_profile() else {
            return;
        };
        let Some(sub) = open.session.current().sub_profiles.get(sub_idx) else {
            return;
        };
        let ti =
            station_by_id(station).map_or_else(|| station.to_string(), |st| st.label.to_string());
        let subtitle = format!(
            "{} input{} on this station",
            station_inputs(station).len(),
            if station_inputs(station).len() == 1 {
                ""
            } else {
                "s"
            },
        );
        let roster: Vec<RosterEntry> = station_inputs(station)
            .iter()
            .map(|name| {
                let inp = yoke_config::catalog::Input::from_csv(name);
                let rows: Vec<_> = sub
                    .bindings()
                    .filter(|b| b.input.as_ref() == Some(&inp))
                    .map(|b| {
                        let output_csv = b.output.to_csv();
                        let glyph = output_glyph(&output_csv);
                        (
                            b.modifier.to_csv(),
                            output_csv,
                            glyph,
                            output_color(palette, &b.output),
                        )
                    })
                    .collect();
                (name.clone(), rows)
            })
            .collect();
        let total: usize = roster.iter().map(|(_, rows)| rows.len()).sum();
        let title = PaneTitle {
            title: ti,
            subtitle,
        };
        (total, title, roster)
    };
    // Immutable borrow on `app` ends here.

    let clear_filter = show_pane_header(ui, palette, &title, total_rows, true);
    ui.separator();
    ui.add_space(4.0);

    let action = show_roster(ui, palette, &roster);
    dispatch_action(app, sub_idx, action);

    if clear_filter {
        app.set_selected_station(None);
    }
}

/// Browse view: list of all bindings across the selected sub-profile. Every row
/// belongs to one sub-profile, so its `(input, modifier)` key is unambiguous and
/// the per-row clear is safe here too.
fn show_all(app: &mut YokeApp, ui: &mut egui::Ui, palette: &crate::theme::Palette, sub_idx: usize) {
    let (title, rows) = {
        let Some(open) = app.open_profile() else {
            return;
        };
        let Some(sub) = open.session.current().sub_profiles.get(sub_idx) else {
            return;
        };
        let title = all_pane_title(sub);
        let rows: Vec<Row> = sub
            .bindings()
            .map(|b| {
                let input_label = b.input.as_ref().map_or_else(
                    || "(unbound)".to_string(),
                    yoke_config::catalog::Input::to_csv,
                );
                let output_label = b.output.to_csv();
                let output_glyph = output_glyph(&output_label);
                let color = output_color(palette, &b.output);
                let modifier = b.modifier.to_csv();
                (input_label, output_label, output_glyph, color, modifier)
            })
            .collect();
        (title, rows)
    };

    let _ = show_pane_header(ui, palette, &title, rows.len(), false);
    ui.separator();
    ui.add_space(4.0);

    let mut action: Option<RosterAction> = None;
    egui::ScrollArea::vertical()
        .id_salt("all_bindings")
        .auto_shrink(false)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.colored_label(palette.ink_3, "No bindings in this sub-profile.");
                });
            }
            for (i, (input_label, output_label, out_glyph, color, modifier)) in
                rows.iter().enumerate()
            {
                let row_id = ui.id().with(("brow", i));
                let hovered = row_hovered(ui, row_id);
                let resp = row_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(a) = binding_row_content(
                            ui,
                            palette,
                            input_label,
                            modifier,
                            output_label,
                            out_glyph,
                            *color,
                            hovered,
                        ) {
                            action = Some(a);
                        }
                    });
                });
                ui.ctx()
                    .data_mut(|d| d.insert_temp(row_id, resp.response.rect));
            }
        });
    dispatch_action(app, sub_idx, action);
}

/// Whether the pointer is over the row painted under `id` last frame. The
/// quiet mod pill needs its row's hover state before the row's rect exists
/// this frame, so it tests the remembered rect; egui repaints on pointer
/// movement, so the one-frame lag is invisible.
fn row_hovered(ui: &egui::Ui, id: egui::Id) -> bool {
    ui.ctx()
        .data(|d| d.get_temp::<egui::Rect>(id))
        .is_some_and(|rect| ui.rect_contains_pointer(rect))
}

/// Render one design `.brow` row: input label, modifier pill (quiet for
/// "normal"), arrow, output button, clear "x". Returns the deferred action
/// (edit-output or clear-one) the user triggered.
#[allow(clippy::too_many_arguments)]
fn binding_row_content(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    input_label: &str,
    modifier: &str,
    output_label: &str,
    output_glyph: &str,
    output_color: egui::Color32,
    row_hovered: bool,
) -> Option<RosterAction> {
    let mut action = None;
    // An input-less binding renders its input label as "(unbound)"; it has no
    // (input, modifier) key, so edit/clear ops can't resolve it. Disable those
    // controls rather than dispatch an op guaranteed to fail with a stray toast.
    let bound = input_label != "(unbound)";

    // Input name (design `.brow-when` now holds only `.when-input`).
    ui.add(
        egui::Label::new(
            egui::RichText::new(input_label)
                .monospace()
                .strong()
                .color(palette.ink_1)
                .size(12.5),
        )
        .truncate(),
    );

    // Modifier pill — quiet for a plain "normal" (design `.mod-pill.quiet`).
    let (pill, pill_color) = mod_pill_style(palette, modifier, row_hovered);
    pill.show(ui, |ui| {
        ui.label(egui::RichText::new(modifier).small().color(pill_color));
    });

    // Arrow.
    ui.label(egui::RichText::new("→").color(palette.ink_3));

    // Filled output button: clicking it re-opens the existing edit-output picker
    // for this exact (input, modifier).
    ui.add_enabled_ui(bound, |ui| {
        if output_button(ui, output_glyph, output_label, output_color).clicked() {
            action = Some(RosterAction::EditOutput {
                input: input_label.to_owned(),
                modifier: modifier.to_owned(),
            });
        }
    });

    // Trailing clear "x": removes this exact (input, modifier) via the existing
    // clear-binding op. Right-aligned so it pins to the row's end.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_enabled_ui(bound, |ui| {
            if clear_button(ui, palette).clicked() {
                action = Some(RosterAction::ClearOne {
                    input: input_label.to_owned(),
                    modifier: modifier.to_owned(),
                });
            }
        });
    });

    action
}

/// A borderless "×" clear control (design `.brow-x`): `ink_3` resting, `ink_1` hover.
fn clear_button(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new("\u{00d7}")
                .size(15.0)
                .color(palette.ink_3),
        )
        .frame(false)
        .min_size(egui::vec2(22.0, 22.0)),
    )
    .on_hover_text("Remove this binding")
}

/// Render an input header row and return any action triggered by the
/// per-input buttons (add / clear-all) or the empty bind button.
fn roster_input_header(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    input: &str,
    has_rows: bool,
) -> Option<RosterAction> {
    let mut action = None;
    ui.add(
        egui::Label::new(
            egui::RichText::new(input)
                .monospace()
                .strong()
                .color(palette.ink_1)
                .size(12.5),
        )
        .truncate(),
    );
    if has_rows {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("clear all").clicked() {
                action = Some(RosterAction::ClearAll {
                    input: input.to_owned(),
                });
            }
            if ui.small_button("add").clicked() {
                action = Some(RosterAction::Add {
                    input: input.to_owned(),
                });
            }
        });
    } else if empty_output_button(ui, input).clicked() {
        action = Some(RosterAction::Add {
            input: input.to_owned(),
        });
    }
    action
}

/// Render one chord sub-row and return any action triggered.
#[allow(clippy::too_many_arguments)]
fn roster_chord_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    input: &str,
    modifier: &str,
    output: &str,
    output_glyph: &str,
    color: egui::Color32,
    row_hovered: bool,
) -> Option<RosterAction> {
    let mut action = None;
    ui.add_space(28.0); // indent to align under the input label
    let (pill, pill_color) = mod_pill_style(palette, modifier, row_hovered);
    if crate::theme::clickable_frame(ui, pill, (input, output, modifier), |ui| {
        ui.label(egui::RichText::new(modifier).small().color(pill_color));
    })
    .clicked()
    {
        action = Some(RosterAction::EditModifier {
            input: input.to_owned(),
            output: output.to_owned(),
            modifier: modifier.to_owned(),
        });
    }
    ui.label(egui::RichText::new("→").color(palette.ink_3));
    if output_button(ui, output_glyph, output, color).clicked() {
        action = Some(RosterAction::EditOutput {
            input: input.to_owned(),
            modifier: modifier.to_owned(),
        });
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if clear_button(ui, palette).clicked() {
            action = Some(RosterAction::ClearOne {
                input: input.to_owned(),
                modifier: modifier.to_owned(),
            });
        }
    });
    action
}

/// Render the scrollable input roster and return the single deferred action
/// (if any) the user triggered. Borrows only palette and display data —
/// `app` is not touched here so the caller can dispatch after.
fn show_roster(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    roster: &[RosterEntry],
) -> Option<RosterAction> {
    let mut action: Option<RosterAction> = None;
    egui::ScrollArea::vertical()
        .id_salt("station_roster")
        .auto_shrink(false)
        .show(ui, |ui| {
            for (input, rows) in roster {
                row_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(a) = roster_input_header(ui, palette, input, !rows.is_empty()) {
                            action = Some(a);
                        }
                    });
                    for (j, (modifier, output, output_glyph, color)) in rows.iter().enumerate() {
                        let row_id = ui.id().with((input.as_str(), j));
                        let hovered = row_hovered(ui, row_id);
                        let resp = ui.horizontal(|ui| {
                            if let Some(a) = roster_chord_row(
                                ui,
                                palette,
                                input,
                                modifier,
                                output,
                                output_glyph,
                                *color,
                                hovered,
                            ) {
                                action = Some(a);
                            }
                        });
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(row_id, resp.response.rect));
                    }
                });
            }
        });
    action
}

/// Dispatch a roster action. The sub-profile index is passed in from the caller
/// (captured at the start of the frame, matching what the user saw).
///
/// # Panics
///
/// Panics if `open_profile` is `None` when a clear action is dispatched.
/// This cannot happen in correct usage: the roster is only rendered when a
/// profile is open.
fn dispatch_action(app: &mut YokeApp, sub_idx: usize, action: Option<RosterAction>) {
    match action {
        Some(RosterAction::ClearOne { input, modifier }) => {
            let r = app
                .edit_session_mut()
                .expect("roster shown with open profile")
                .clear_binding(sub_idx, &input, Some(&modifier));
            app.report_edit(r);
        }
        Some(RosterAction::ClearAll { input }) => {
            let r = app
                .edit_session_mut()
                .expect("roster shown with open profile")
                .clear_binding(sub_idx, &input, None);
            app.report_edit(r);
        }
        Some(RosterAction::Add { input }) => {
            app.open_picker(PickerTarget::AddBinding { input });
        }
        Some(RosterAction::EditOutput { input, modifier }) => {
            app.open_picker(PickerTarget::EditOutput { input, modifier });
        }
        Some(RosterAction::EditModifier {
            input,
            output,
            modifier,
        }) => {
            app.open_picker(PickerTarget::EditModifier {
                input,
                output,
                modifier,
            });
        }
        None => {}
    }
}

/// A short display glyph for an output, derived from its CSV id. Known outputs
/// map to the design's compact codes (`LMB`, `Wh↑`, cursor arrows, gamepad
/// codes); everything else falls back to the leading token of the id so the
/// glyph is always derived from real data, never fabricated.
pub(crate) fn output_glyph(csv: &str) -> String {
    let mapped = match csv {
        "mouse_left_button" => "LMB",
        "mouse_right_button" => "RMB",
        "mouse_middle_button" => "MMB",
        "mouse_left" => "◀",
        "mouse_right" => "▶",
        "mouse_up" => "▲",
        "mouse_down" => "▼",
        "mouse_wheel_up" => "Wh↑",
        "mouse_wheel_down" => "Wh↓",
        "mouse_pan_left" => "Pan←",
        "mouse_pan_right" => "Pan→",
        "x" => "╳",
        "circle" => "○",
        "square" => "□",
        "triangle" => "△",
        "select" => "⊟",
        "start" => "≡",
        "ps3" => "PS",
        "touch" => "⊙",
        "increment_mode" => "M+",
        "decrement_mode" => "M-",
        _ => "",
    };
    if !mapped.is_empty() {
        return mapped.to_owned();
    }
    // Keyboard keys: drop the `kb_` prefix and show a compact form.
    if let Some(key) = csv.strip_prefix("kb_") {
        return short_token(key);
    }
    // D-pad direction (`dpad_NE`) -> the cardinal letters.
    if let Some(dir) = csv.strip_prefix("dpad_") {
        return dir.to_owned();
    }
    // Joystick axes, gamepad letter buttons, and any unknown id: leading token.
    short_token(csv)
}

/// Reduce a snake/word token to a compact glyph: a recognized short form for a
/// handful of common tokens, else the uppercased leading 1-3 characters of the
/// first word. Pure formatting over an existing id — introduces no new meaning.
fn short_token(token: &str) -> String {
    match token {
        "left_shift" | "right_shift" => return "⇧".to_owned(),
        "left_control" | "right_control" => return "⌃".to_owned(),
        "left_alt" | "right_alt" => return "⌥".to_owned(),
        "left_gui" | "right_gui" => return "⌘".to_owned(),
        "space" => return "␣".to_owned(),
        "enter" => return "⏎".to_owned(),
        "escape" => return "esc".to_owned(),
        "tab" => return "⇥".to_owned(),
        "backspace" => return "⌫".to_owned(),
        "delete" => return "⌦".to_owned(),
        "up_arrow" => return "↑".to_owned(),
        "down_arrow" => return "↓".to_owned(),
        "left_arrow" => return "←".to_owned(),
        "right_arrow" => return "→".to_owned(),
        _ => {}
    }
    let first = token.split(['_', ' ']).next().unwrap_or(token);
    if first.chars().count() <= 3 {
        first.to_uppercase()
    } else {
        first.chars().take(3).collect::<String>().to_uppercase()
    }
}

#[cfg(test)]
mod tests {
    use super::{output_glyph, short_token};

    #[test]
    fn output_glyph_maps_known_mouse_codes() {
        assert_eq!(output_glyph("mouse_left_button"), "LMB");
        assert_eq!(output_glyph("mouse_wheel_up"), "Wh↑");
    }

    #[test]
    fn output_glyph_keyboard_uses_compact_token() {
        assert_eq!(output_glyph("kb_escape"), "esc");
        assert_eq!(output_glyph("kb_a"), "A");
        assert_eq!(output_glyph("kb_tab"), "⇥");
    }

    #[test]
    fn output_glyph_dpad_keeps_cardinal() {
        assert_eq!(output_glyph("dpad_NE"), "NE");
    }

    #[test]
    fn output_glyph_unknown_falls_back_to_leading_token() {
        // No fabricated glyph: an unrecognized id yields its own leading token.
        assert_eq!(output_glyph("mystery_output"), "MYS");
    }

    #[test]
    fn short_token_maps_modifiers_and_truncates_long_words() {
        assert_eq!(short_token("left_shift"), "⇧");
        assert_eq!(short_token("mouthpiece"), "MOU");
        assert_eq!(short_token("lip"), "LIP");
    }
}
