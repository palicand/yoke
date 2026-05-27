use crate::app::YokeApp;
use crate::stations::input_belongs_to;
use crate::theme::output_color;

type Row = (String, String, egui::Color32, Option<String>);

pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let Some(open) = app.open_profile() else { return };
    let Some(sub) = open.profile.sub_profiles.get(app.selected_subprofile()) else { return };
    let palette = *app.palette();
    let filter = app.selected_station();

    let title = filter
        .map_or_else(|| "Bindings - all".to_string(), |station| format!("Bindings - {station}"));

    // Collect display rows while `sub` borrow is live, releasing it before
    // any `&mut app` call.
    let rows: Vec<Row> = sub
        .bindings()
        .filter(|b| {
            filter.is_none_or(|station| {
                b.input.as_ref().and_then(input_belongs_to) == Some(station)
            })
        })
        .map(|b| {
            let input_label = b
                .input
                .as_ref()
                .map_or_else(|| "(unbound)".to_string(), yoke_config::catalog::Input::to_csv);
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

    // `sub` borrow ends here. Header can now capture `clear_filter` freely.
    let mut clear_filter = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong());
        if filter.is_some() && ui.small_button("Clear filter").clicked() {
            clear_filter = true;
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (input_label, output_label, color, modifier_label) in &rows {
            ui.horizontal(|ui| {
                ui.colored_label(palette.ink_2, input_label);
                ui.label("->");
                ui.colored_label(*color, output_label);
                if let Some(modifier) = modifier_label {
                    ui.label(egui::RichText::new(modifier).small().color(palette.ink_3));
                }
            });
        }
        if rows.is_empty() {
            ui.colored_label(palette.ink_3, "No bindings for this station.");
        }
    });

    // Safe: `sub` borrow released above; `&mut app` is uncontested here.
    if clear_filter {
        app.set_selected_station(None);
    }
}
