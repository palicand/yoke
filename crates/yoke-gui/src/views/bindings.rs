use crate::app::{PickerTarget, YokeApp};
use crate::stations::{StationKind, station_by_id, station_inputs};
use crate::theme::{output_color, pill_frame, row_frame};

// Row tuple for the unfiltered browse view: input label, output label, color, optional modifier.
type Row = (String, String, egui::Color32, Option<String>);

// (input_csv, chord_rows): each chord row is (modifier_csv, output_csv, color).
type RosterEntry = (String, Vec<(String, String, egui::Color32)>);

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

/// Station view: full input roster with edit affordances.
fn show_station(
    app: &mut YokeApp,
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    sub_idx: usize,
    station: &'static str,
) {
    // Collect all display data while the immutable `app` borrow is live.
    let (total_rows, eyebrow, title, roster) = {
        let Some(open) = app.open_profile() else {
            return;
        };
        let Some(sub) = open.session.current().sub_profiles.get(sub_idx) else {
            return;
        };
        let (ey, ti) = station_by_id(station).map_or_else(
            || ("STATION", station.to_string()),
            |st| (kind_eyebrow(st.kind), st.label.to_string()),
        );
        let roster: Vec<RosterEntry> = station_inputs(station)
            .iter()
            .map(|name| {
                let inp = yoke_config::catalog::Input::from_csv(name);
                let rows: Vec<_> = sub
                    .bindings()
                    .filter(|b| b.input.as_ref() == Some(&inp))
                    .map(|b| {
                        (
                            b.modifier.to_csv(),
                            b.output.to_csv(),
                            output_color(palette, &b.output),
                        )
                    })
                    .collect();
                (name.clone(), rows)
            })
            .collect();
        let total: usize = roster.iter().map(|(_, rows)| rows.len()).sum();
        (total, ey, ti, roster)
    };
    // Immutable borrow on `app` ends here.

    let mut clear_filter = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(eyebrow).small().color(palette.ink_3));
            ui.label(egui::RichText::new(&title).size(20.0).strong());
            ui.label(egui::RichText::new(format!("{total_rows} bindings")).color(palette.ink_2));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if ui.small_button("Show all").clicked() {
                clear_filter = true;
            }
        });
    });
    ui.separator();
    ui.add_space(4.0);

    let action = show_roster(ui, palette, &roster);
    dispatch_action(app, sub_idx, action);

    if clear_filter {
        app.set_selected_station(None);
    }
}

/// Browse view: read-only list of all bindings across the sub-profile.
fn show_all(app: &YokeApp, ui: &mut egui::Ui, palette: &crate::theme::Palette, sub_idx: usize) {
    let rows: Vec<Row> = {
        let Some(open) = app.open_profile() else {
            return;
        };
        let Some(sub) = open.session.current().sub_profiles.get(sub_idx) else {
            return;
        };
        sub.bindings()
            .map(|b| {
                let input_label = b.input.as_ref().map_or_else(
                    || "(unbound)".to_string(),
                    yoke_config::catalog::Input::to_csv,
                );
                let output_label = b.output.to_csv();
                let color = output_color(palette, &b.output);
                let modifier = b.modifier.to_csv();
                let modifier_label = if modifier.is_empty() || modifier == "normal" {
                    None
                } else {
                    Some(modifier)
                };
                (input_label, output_label, color, modifier_label)
            })
            .collect()
    };

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("ALL").small().color(palette.ink_3));
            ui.label(egui::RichText::new("All bindings").size(20.0).strong());
            ui.label(egui::RichText::new(format!("{} bindings", rows.len())).color(palette.ink_2));
        });
    });
    ui.separator();
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.colored_label(palette.ink_3, "No bindings in this sub-profile.");
                });
            }
            for (input_label, output_label, color, modifier_label) in &rows {
                row_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("WHEN").small().color(palette.ink_3));
                            ui.label(
                                egui::RichText::new(input_label)
                                    .monospace()
                                    .strong()
                                    .color(palette.ink_1),
                            );
                        });
                        if let Some(modifier) = modifier_label {
                            pill_frame().show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(modifier).small().color(palette.ink_2),
                                );
                            });
                        }
                        ui.label(egui::RichText::new("->").color(palette.ink_3));
                        ui.colored_label(*color, output_label);
                    });
                });
            }
        });
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
        .auto_shrink(false)
        .show(ui, |ui| {
            for (input, rows) in roster {
                row_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(input.as_str())
                                .monospace()
                                .strong()
                                .color(palette.ink_1),
                        );
                        if rows.is_empty() {
                            ui.colored_label(palette.ink_3, "(unbound)");
                            if ui.small_button("set").clicked() {
                                action = Some(RosterAction::Add {
                                    input: input.clone(),
                                });
                            }
                        } else {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("clear all").clicked() {
                                        action = Some(RosterAction::ClearAll {
                                            input: input.clone(),
                                        });
                                    }
                                    if ui.small_button("add").clicked() {
                                        action = Some(RosterAction::Add {
                                            input: input.clone(),
                                        });
                                    }
                                },
                            );
                        }
                    });
                    for (modifier, output, color) in rows {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            pill_frame().show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(modifier).small().color(palette.ink_2),
                                );
                            });
                            ui.label(egui::RichText::new("->").color(palette.ink_3));
                            ui.colored_label(*color, output);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("x").clicked() {
                                        action = Some(RosterAction::ClearOne {
                                            input: input.clone(),
                                            modifier: modifier.clone(),
                                        });
                                    }
                                    if ui.small_button("modifier").clicked() {
                                        action = Some(RosterAction::EditModifier {
                                            input: input.clone(),
                                            output: output.clone(),
                                            modifier: modifier.clone(),
                                        });
                                    }
                                    if ui.small_button("output").clicked() {
                                        action = Some(RosterAction::EditOutput {
                                            input: input.clone(),
                                            modifier: modifier.clone(),
                                        });
                                    }
                                },
                            );
                        });
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

const fn kind_eyebrow(kind: StationKind) -> &'static str {
    match kind {
        StationKind::Joystick => "JOYSTICK",
        StationKind::Mouthpiece => "MOUTHPIECE",
        StationKind::Lip => "LIP",
        StationKind::Side => "SIDE TUBE",
    }
}
