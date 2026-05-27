use crate::app::YokeApp;
use crate::stations::{StationKind, input_belongs_to, station_by_id};
use crate::theme::{output_color, pill_frame, row_frame};

type Row = (String, String, egui::Color32, Option<String>);

pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let Some(open) = app.open_profile() else {
        return;
    };
    let Some(sub) = open.profile.sub_profiles.get(app.selected_subprofile()) else {
        return;
    };
    let palette = *app.palette();
    let filter = app.selected_station();

    // Collect display rows while `sub` borrow is live, releasing it before
    // any `&mut app` call.
    let rows: Vec<Row> = sub
        .bindings()
        .filter(|b| {
            filter
                .is_none_or(|station| b.input.as_ref().and_then(input_belongs_to) == Some(station))
        })
        .map(|b| {
            let input_label = b.input.as_ref().map_or_else(
                || "(unbound)".to_string(),
                yoke_config::catalog::Input::to_csv,
            );
            let output_label = b.output.to_csv();
            let color = output_color(&palette, &b.output);
            let modifier = b.modifier.to_csv();
            let modifier_label = if modifier.is_empty() || modifier == "normal" {
                None
            } else {
                Some(modifier)
            };
            (input_label, output_label, color, modifier_label)
        })
        .collect();

    // `sub` borrow ends here.
    let (eyebrow, title) = filter.and_then(station_by_id).map_or_else(
        || ("ALL", "All bindings".to_string()),
        |st| (kind_eyebrow(st.kind), st.label.to_string()),
    );

    let mut clear_filter = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(eyebrow).small().color(palette.ink_3));
            ui.label(egui::RichText::new(&title).size(20.0).strong());
            ui.label(egui::RichText::new(format!("{} bindings", rows.len())).color(palette.ink_2));
        });
        if filter.is_some() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.small_button("Show all").clicked() {
                    clear_filter = true;
                }
            });
        }
    });
    ui.separator();
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            if rows.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.colored_label(palette.ink_3, "No bindings for this station.");
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

    // Safe: `sub` borrow released above; `&mut app` is uncontested here.
    if clear_filter {
        app.set_selected_station(None);
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
