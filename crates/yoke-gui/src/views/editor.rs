use egui::text::LayoutJob;
use egui::{FontFamily, FontId, TextFormat};
use yoke_config::catalog::{Channel, SubProfileMode};
use yoke_config::model::SubProfile;

use crate::app::{SubProfileUi, YokeApp};
use crate::theme::{Palette, card_frame, strip_frame};

// One sub-profile chip: display name (the mode) + binding count.
type Tab = (String, usize);

// Deferred action from the strip; dispatched after all immutable borrows end.
enum StripAction {
    Add {
        name: String,
        mode: SubProfileMode,
        sub_mode: String,
        channel: Channel,
    },
    Clone {
        index: usize,
        to: String,
    },
    Rename {
        index: usize,
        to: String,
    },
    Delete {
        index: usize,
    },
}

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

    // Always show the strip — management requires it even for a single layer.
    let strip_action = show_strip(app, ui, &palette, &tabs);
    dispatch_strip_action(app, strip_action);

    ui.add_space(10.0);

    ui.columns(2, |cols| {
        card_frame().show(&mut cols[0], |ui| crate::views::map::show(app, ui));
        card_frame().show(&mut cols[1], |ui| crate::views::bindings::show(app, ui));
    });
}

/// Render the sub-profile chip strip and any inline management form.
/// Returns a deferred action if the user triggered one.
fn show_strip(
    app: &mut YokeApp,
    ui: &mut egui::Ui,
    palette: &Palette,
    tabs: &[Tab],
) -> Option<StripAction> {
    let selected = app.selected_subprofile();
    // Clone the UI state so we can mutate it without a live borrow on `app`.
    let ui_state = app.subprofile_ui().clone();

    let mut action: Option<StripAction> = None;
    let mut new_ui_state: Option<SubProfileUi> = None;
    let mut new_selection: Option<usize> = None;

    strip_frame().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            show_chips(
                app,
                ui,
                palette,
                tabs,
                selected,
                &ui_state,
                &mut action,
                &mut new_ui_state,
                &mut new_selection,
            );
        });

        // Inline forms — rendered below the chip row inside the same frame.
        match &ui_state {
            SubProfileUi::Renaming { index, value } => {
                show_rename_form(ui, *index, value, &mut action, &mut new_ui_state);
            }
            SubProfileUi::Adding {
                name,
                mode,
                sub_mode,
                channel,
            } => {
                show_add_form(
                    ui,
                    name,
                    *mode,
                    sub_mode,
                    *channel,
                    &mut action,
                    &mut new_ui_state,
                );
            }
            SubProfileUi::Closed => {}
        }
    });

    // Apply UI state transitions that don't go through dispatch (selection
    // changes, cancel, field edits). Dispatch handles the rest via
    // set_subprofile_ui(Closed) after a successful engine op.
    if action.is_none()
        && let Some(s) = new_ui_state
    {
        app.set_subprofile_ui(s);
    }
    if let Some(i) = new_selection {
        app.set_selected_subprofile(i);
        app.set_selected_station(None);
    }

    action
}

/// Render the chip row (tabs + per-selected-chip buttons + "+" chip).
#[allow(clippy::too_many_arguments)]
fn show_chips(
    app: &YokeApp,
    ui: &mut egui::Ui,
    palette: &Palette,
    tabs: &[Tab],
    selected: usize,
    ui_state: &SubProfileUi,
    action: &mut Option<StripAction>,
    new_ui_state: &mut Option<SubProfileUi>,
    new_selection: &mut Option<usize>,
) {
    for (i, (name, count)) in tabs.iter().enumerate() {
        let is_selected = selected == i;
        let job = tab_label(name, *count, palette);
        if ui.selectable_label(is_selected, job).clicked() && !is_selected {
            *new_selection = Some(i);
            // Closing any open inline form when selecting a different chip is
            // intentional: the form targets the previously selected index and
            // switching chip identity invalidates it.
            *new_ui_state = Some(SubProfileUi::Closed);
        }

        // Per-chip action buttons: shown only for the selected chip when no
        // inline form is open.
        if is_selected && matches!(ui_state, SubProfileUi::Closed) {
            show_chip_buttons(app, ui, i, action, new_ui_state);
        }
    }

    // "+" chip — switches to the Adding form.
    if matches!(ui_state, SubProfileUi::Closed)
        && ui
            .selectable_label(false, egui::RichText::new("+").strong())
            .clicked()
    {
        *new_ui_state = Some(SubProfileUi::Adding {
            name: String::new(),
            mode: 0,
            sub_mode: "Normal".into(),
            channel: 0,
        });
    }
}

/// Render the rename/clone/delete buttons for a selected chip.
fn show_chip_buttons(
    app: &YokeApp,
    ui: &mut egui::Ui,
    index: usize,
    action: &mut Option<StripAction>,
    new_ui_state: &mut Option<SubProfileUi>,
) {
    if ui.small_button("rename").clicked() {
        *new_ui_state = Some(SubProfileUi::Renaming {
            index,
            // Seed with the raw profile_name (not the display label) so
            // the user edits what the engine stores.
            value: app
                .open_profile()
                .and_then(|o| o.session.current().sub_profiles.get(index))
                .map_or_else(String::new, |s| s.header.profile_name.clone()),
        });
    }
    if ui.small_button("clone").clicked() {
        // Read the current name at action-build time — the index the user saw
        // must match the index dispatched.
        let current_name = app
            .open_profile()
            .and_then(|o| o.session.current().sub_profiles.get(index))
            .map_or("", |s| s.header.profile_name.as_str());
        let to = if current_name.is_empty() {
            String::new()
        } else {
            format!("{current_name} copy")
        };
        *action = Some(StripAction::Clone { index, to });
    }
    if ui.small_button("delete").clicked() {
        *action = Some(StripAction::Delete { index });
    }
}

/// Render the inline rename form. Writes to `action` on commit; writes to
/// `new_ui_state` on the explicit cancel button or to persist the in-progress
/// buffer. Escape-cancel is handled by the global Escape chain in `app.rs`,
/// which closes the form before it can reach `close_profile`.
fn show_rename_form(
    ui: &mut egui::Ui,
    index: usize,
    value: &str,
    action: &mut Option<StripAction>,
    new_ui_state: &mut Option<SubProfileUi>,
) {
    let mut buf = value.to_owned();
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Rename:");
        let resp = ui.text_edit_singleline(&mut buf);
        // Request focus on every frame while the form is open so the user can
        // type immediately without clicking the field first.
        resp.request_focus();
        let commit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if commit || ui.small_button("ok").clicked() {
            *action = Some(StripAction::Rename {
                index,
                to: buf.clone(),
            });
            // set_subprofile_ui(Closed) is handled in dispatch_strip_action
        } else if ui.small_button("cancel").clicked() {
            *new_ui_state = Some(SubProfileUi::Closed);
        } else {
            // Keep the edited buffer live — write it back so next frame the
            // text field retains the user's typing.
            *new_ui_state = Some(SubProfileUi::Renaming { index, value: buf });
        }
    });
}

/// Render the inline add-sub-profile form. Writes to `action` on submit;
/// writes to `new_ui_state` on cancel or to persist in-progress fields.
#[allow(clippy::too_many_arguments)]
fn show_add_form(
    ui: &mut egui::Ui,
    name: &str,
    mode: usize,
    sub_mode: &str,
    channel: usize,
    action: &mut Option<StripAction>,
    new_ui_state: &mut Option<SubProfileUi>,
) {
    let mut name_buf = name.to_owned();
    let mut mode_idx = mode;
    let mut sub_mode_buf = sub_mode.to_owned();
    let mut channel_idx = channel;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut name_buf);
    });
    ui.horizontal(|ui| {
        ui.label("Mode:");
        egui::ComboBox::from_id_salt("strip_add_mode")
            .selected_text(SubProfileMode::KNOWN[mode_idx].canonical_csv())
            .show_ui(ui, |ui| {
                for (i, m) in SubProfileMode::KNOWN.iter().enumerate() {
                    ui.selectable_value(&mut mode_idx, i, m.canonical_csv());
                }
            });
        ui.label("Sub-mode:");
        ui.text_edit_singleline(&mut sub_mode_buf);
        ui.label("Channel:");
        egui::ComboBox::from_id_salt("strip_add_channel")
            .selected_text(Channel::ALL[channel_idx].canonical_csv())
            .show_ui(ui, |ui| {
                for (i, c) in Channel::ALL.iter().enumerate() {
                    ui.selectable_value(&mut channel_idx, i, c.canonical_csv());
                }
            });
    });
    ui.horizontal(|ui| {
        if ui.button("add").clicked() {
            *action = Some(StripAction::Add {
                name: name_buf.clone(),
                mode: SubProfileMode::KNOWN[mode_idx].clone(),
                sub_mode: sub_mode_buf.clone(),
                channel: Channel::ALL[channel_idx],
            });
            // set_subprofile_ui(Closed) handled in dispatch_strip_action
        } else if ui.button("cancel").clicked() {
            *new_ui_state = Some(SubProfileUi::Closed);
        } else {
            // Preserve the user's in-progress form fields.
            *new_ui_state = Some(SubProfileUi::Adding {
                name: name_buf,
                mode: mode_idx,
                sub_mode: sub_mode_buf,
                channel: channel_idx,
            });
        }
    });
}

/// Dispatch a strip action after all borrows from the rendering pass have ended.
///
/// # Panics
///
/// Panics if `open_profile` is `None` — the strip is only rendered when a
/// profile is open, so this cannot fire in correct usage.
#[allow(clippy::missing_panics_doc)]
fn dispatch_strip_action(app: &mut YokeApp, action: Option<StripAction>) {
    match action {
        Some(StripAction::Add {
            name,
            mode,
            sub_mode,
            channel,
        }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .add_sub_profile(&name, mode, &sub_mode, channel);
            if r.is_ok() {
                let last = app
                    .open_profile()
                    .expect("open")
                    .session
                    .current()
                    .sub_profiles
                    .len()
                    - 1;
                app.set_selected_subprofile(last);
            }
            app.report_edit(r);
            app.set_subprofile_ui(SubProfileUi::Closed);
        }
        Some(StripAction::Clone { index, to }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .clone_sub_profile(index, &to);
            app.report_edit(r);
        }
        Some(StripAction::Rename { index, to }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .rename_sub_profile(index, &to);
            app.report_edit(r);
            app.set_subprofile_ui(SubProfileUi::Closed);
        }
        Some(StripAction::Delete { index }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .delete_sub_profile(index);
            if r.is_ok() {
                let len = app
                    .open_profile()
                    .expect("open")
                    .session
                    .current()
                    .sub_profiles
                    .len();
                // Selection must never point past the shrunken strip.
                let clamped = app.selected_subprofile().min(len - 1);
                app.set_selected_subprofile(clamped);
            }
            // LastSubProfileDeletion surfaces as a toast; state is untouched.
            app.report_edit(r);
        }
        None => {}
    }
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{StripAction, dispatch_strip_action};
    use crate::app::SubProfileUi;
    use crate::app::test_support::{a_profile, open_app_with, two_sub_profile};
    use yoke_config::catalog::{Channel, SubProfileMode};

    fn sub_count(app: &crate::app::YokeApp) -> usize {
        app.open_profile()
            .map_or(0, |o| o.session.current().sub_profiles.len())
    }

    #[test]
    fn dispatch_delete_clamps_selection_to_last() {
        let mut app = open_app_with(two_sub_profile());
        // The user is on the second (index 1) sub-profile and deletes it; the
        // strip shrinks to one layer, so selection must clamp to 0.
        app.set_selected_subprofile(1);
        dispatch_strip_action(&mut app, Some(StripAction::Delete { index: 1 }));
        assert_eq!(sub_count(&app), 1);
        assert_eq!(app.selected_subprofile(), 0);
    }

    #[test]
    fn dispatch_delete_last_toasts_and_leaves_state_intact() {
        let mut app = open_app_with(a_profile());
        app.set_selected_subprofile(0);
        dispatch_strip_action(&mut app, Some(StripAction::Delete { index: 0 }));
        // Engine refuses LastSubProfileDeletion: profile and selection untouched,
        // refusal surfaced as a toast.
        assert_eq!(sub_count(&app), 1);
        assert_eq!(app.selected_subprofile(), 0);
        assert!(app.has_toast(), "refusal must surface as toast");
    }

    #[test]
    fn dispatch_add_selects_new_last_and_closes_form() {
        let mut app = open_app_with(a_profile());
        app.set_selected_subprofile(0);
        app.set_subprofile_ui(SubProfileUi::Adding {
            name: "Extra".into(),
            mode: 0,
            sub_mode: "Normal".into(),
            channel: 0,
        });
        dispatch_strip_action(
            &mut app,
            Some(StripAction::Add {
                name: "Extra".into(),
                mode: SubProfileMode::Mouse,
                sub_mode: "Normal".into(),
                channel: Channel::Usb,
            }),
        );
        assert_eq!(sub_count(&app), 2);
        // The newly appended layer (last index) becomes selected.
        assert_eq!(app.selected_subprofile(), 1);
        // The inline form closes after a successful add.
        assert!(matches!(app.subprofile_ui(), SubProfileUi::Closed));
    }

    #[test]
    fn dispatch_rename_renames_indexed_layer_and_closes_form() {
        let mut app = open_app_with(two_sub_profile());
        app.set_subprofile_ui(SubProfileUi::Renaming {
            index: 1,
            value: "Cougar".into(),
        });
        dispatch_strip_action(
            &mut app,
            Some(StripAction::Rename {
                index: 1,
                to: "Cougar".into(),
            }),
        );
        let subs = &app
            .open_profile()
            .expect("open")
            .session
            .current()
            .sub_profiles;
        // Only the indexed layer's name changes; the other layer's name is
        // untouched (the fixture's profile_name column is empty for both).
        assert_eq!(subs[1].header.profile_name, "Cougar");
        assert_eq!(subs[0].header.profile_name, "");
        // The inline form closes after a successful rename.
        assert!(matches!(app.subprofile_ui(), SubProfileUi::Closed));
    }

    #[test]
    fn dispatch_clone_appends_copy_leaving_original_untouched() {
        let mut app = open_app_with(two_sub_profile());
        // Name index 1 so the "{name} copy" derivation has something to extend,
        // then clone it.
        dispatch_strip_action(
            &mut app,
            Some(StripAction::Rename {
                index: 1,
                to: "Left Analog".into(),
            }),
        );
        dispatch_strip_action(
            &mut app,
            Some(StripAction::Clone {
                index: 1,
                to: "Left Analog copy".into(),
            }),
        );
        let subs = &app
            .open_profile()
            .expect("open")
            .session
            .current()
            .sub_profiles;
        assert_eq!(subs.len(), 3);
        // The clone is appended at the end with the derived "{name} copy" name.
        assert_eq!(subs[2].header.profile_name, "Left Analog copy");
        // The original layer is untouched.
        assert_eq!(subs[1].header.profile_name, "Left Analog");
    }
}
