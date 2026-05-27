use crate::app::YokeApp;

/// # Panics
///
/// Panics if called without an open profile — the caller must only route here
/// when `app.open_profile().is_some()`.
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    // Pre-read owned values while the immutable borrow of `app` is live so
    // the borrow ends before any `&mut app` call below.
    let (breadcrumb, title) = {
        let open = app
            .open_profile()
            .expect("editor shown with an open profile");
        (
            open.source.breadcrumb(),
            open.profile.top_line.title.clone(),
        )
    };
    let ink_2 = app.palette().ink_2;

    let mut go_back = false;
    ui.horizontal(|ui| {
        if ui.button("< Back").clicked() {
            go_back = true;
        }
        ui.label(egui::RichText::new(breadcrumb).color(ink_2));
    });
    if go_back {
        app.close_profile();
        return;
    }
    ui.heading(title);
    ui.separator();

    // Sub-profile strip — only shown when there are multiple sub-profiles.
    let sub_labels: Vec<String> = app
        .open_profile()
        .map(|op| {
            op.profile
                .sub_profiles
                .iter()
                .map(|s| s.header.profile_name.clone())
                .collect()
        })
        .unwrap_or_default();
    if sub_labels.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            for (i, label) in sub_labels.iter().enumerate() {
                let selected = app.selected_subprofile() == i;
                if ui.selectable_label(selected, label).clicked() {
                    app.set_selected_subprofile(i);
                    app.set_selected_station(None);
                }
            }
        });
        ui.separator();
    }

    // Two-column layout: device map (left) + bindings (right).
    ui.columns(2, |cols| {
        crate::views::map::show(app, &mut cols[0]);
        crate::views::bindings::show(app, &mut cols[1]);
    });
}
