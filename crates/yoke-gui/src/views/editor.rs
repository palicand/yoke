use egui::text::LayoutJob;
use egui::{FontFamily, FontId, TextFormat};
use yoke_config::model::SubProfile;

use crate::app::YokeApp;
use crate::theme::{Palette, card_frame, strip_frame};

// One sub-profile chip: display name (the mode) + binding count.
type Tab = (String, usize);

/// # Panics
///
/// Panics if called without an open profile — the caller must only route here
/// when `app.open_profile().is_some()`.
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    // Pre-read owned values while the immutable borrow of `app` is live so
    // the borrow ends before any `&mut app` call below.
    let (breadcrumb, title, sub_count, total_bindings, tabs) = {
        let open = app
            .open_profile()
            .expect("editor shown with an open profile");
        let subs = &open.session.current().sub_profiles;
        let total: usize = subs.iter().map(|s| s.bindings().count()).sum();
        let tabs: Vec<Tab> = subs
            .iter()
            .enumerate()
            .map(|(i, s)| (sub_label(s, i), s.bindings().count()))
            .collect();
        (
            open.source.breadcrumb(),
            open.session.current().top_line.title.clone(),
            subs.len(),
            total,
            tabs,
        )
    };
    let palette = *app.palette();

    let mut go_back = false;
    ui.horizontal(|ui| {
        if ui.button("< Back").clicked() {
            go_back = true;
        }
        ui.add_space(2.0);
        ui.label(egui::RichText::new(breadcrumb).small().color(palette.ink_3));
    });
    if go_back {
        app.close_profile();
        return;
    }

    ui.heading(title);
    ui.horizontal(|ui| {
        stat(ui, &palette, total_bindings, "bindings");
        ui.add_space(8.0);
        stat(ui, &palette, sub_count, "sub-profiles");
    });
    ui.add_space(10.0);

    if tabs.len() > 1 {
        strip_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, (name, count)) in tabs.iter().enumerate() {
                    let selected = app.selected_subprofile() == i;
                    let job = tab_label(name, *count, &palette);
                    if ui.selectable_label(selected, job).clicked() {
                        app.set_selected_subprofile(i);
                        app.set_selected_station(None);
                    }
                }
            });
        });
        ui.add_space(10.0);
    }

    ui.columns(2, |cols| {
        card_frame().show(&mut cols[0], |ui| crate::views::map::show(app, ui));
        card_frame().show(&mut cols[1], |ui| crate::views::bindings::show(app, ui));
    });
}

fn stat(ui: &mut egui::Ui, palette: &Palette, n: usize, label: &str) {
    ui.label(
        egui::RichText::new(n.to_string())
            .monospace()
            .strong()
            .color(palette.ink_1),
    );
    ui.label(egui::RichText::new(label).color(palette.ink_2));
}

fn tab_label(name: &str, count: usize, palette: &Palette) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(
        name,
        0.0,
        TextFormat {
            font_id: FontId::new(13.0, FontFamily::Proportional),
            color: palette.ink_1,
            ..Default::default()
        },
    );
    job.append(
        &format!("  {count}"),
        0.0,
        TextFormat {
            font_id: FontId::new(11.0, FontFamily::Monospace),
            color: palette.ink_3,
            ..Default::default()
        },
    );
    job
}

// The sub-profile's display name: the explicit profile name if present, else
// the mode (e.g. "Left Analog" / "Mixed joy"), falling back to an index only
// when neither carries a label. The sub-mode (Normal / Alternate / ...) is
// appended when present, which is what distinguishes same-mode layers.
fn sub_label(s: &SubProfile, i: usize) -> String {
    let base = {
        let name = s.header.profile_name.trim();
        if name.is_empty() {
            let mode = s.header.mode.canonical_csv();
            if mode.trim().is_empty() {
                format!("Sub-profile {}", i + 1)
            } else {
                mode
            }
        } else {
            name.to_owned()
        }
    };
    let sub = s.header.sub_mode.trim();
    if sub.is_empty() {
        base
    } else {
        format!("{base} · {sub}")
    }
}
