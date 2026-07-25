use yoke_config::catalog::{Channel, SubProfileMode};
use yoke_config::model::SubProfile;

use crate::app::{SubProfileUi, YokeApp};
use crate::theme::{self, Palette, card_frame, strip_frame};

// (name, sublabel, binding count). The sublabel feeds only the selected-layer
// caption below the strip, not the chip itself.
type Tab = (String, String, usize);

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

// Snapshot of session/source state the header toolbar renders from, pre-read
// so the immutable borrow of `app` ends before dispatch.
// The bools are independent affordance gates, not a disguised state machine.
#[allow(clippy::struct_excessive_bools)]
struct HeaderState {
    breadcrumb: String,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
    is_community: bool,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    volume_present: bool,
    save_in_flight: bool,
}

// Deferred toolbar actions; dispatched after the render closure ends.
// Multiple flags can be set in one frame, so this is not an enum.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct ToolbarActions {
    go_back: bool,
    undo: bool,
    redo: bool,
    preview: bool,
    save: bool,
    #[cfg(not(target_arch = "wasm32"))]
    save_as: bool,
    #[cfg(not(target_arch = "wasm32"))]
    save_to_qs: bool,
}

/// # Panics
///
/// Panics if called without an open profile — the caller must only route here
/// when `app.open_profile().is_some()`.
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    // Pre-read owned values while the immutable borrow of `app` is live so
    // the borrow ends before any `&mut app` call below.
    let (header, title, total_bindings, tabs) = {
        let open = app
            .open_profile()
            .expect("editor shown with an open profile");
        let subs = &open.session.current().sub_profiles;
        let total: usize = subs.iter().map(|s| s.bindings().count()).sum();
        let tabs: Vec<Tab> = subs
            .iter()
            .enumerate()
            .map(|(i, s)| (sub_base_label(s, i), sub_sublabel(s), s.bindings().count()))
            .collect();
        let header = HeaderState {
            breadcrumb: open.source.breadcrumb(),
            dirty: open.session.is_dirty(),
            can_undo: open.session.can_undo(),
            can_redo: open.session.can_redo(),
            is_community: matches!(open.source, crate::state::ProfileSource::Community { .. }),
            volume_present: app.volume_present(),
            save_in_flight: app.save_in_flight(),
        };
        (
            header,
            open.session.current().top_line.title.clone(),
            total,
            tabs,
        )
    };
    let palette = *app.palette();

    let actions = show_header(ui, &header, &title, total_bindings, &palette);
    if actions.go_back {
        app.request_close_profile();
        return;
    }
    dispatch_toolbar(app, &actions);

    ui.add_space(8.0);

    // Always show the strip — management requires it even for a single layer.
    let strip_action = show_strip(app, ui, &palette, &tabs);
    dispatch_strip_action(app, strip_action);

    ui.add_space(8.0);

    // Explicit child rects, not allocate_ui: a child allocated inside a
    // horizontal layout inherits its left-to-right flow and ignores the desired
    // width once content overflows.
    let gap = 16.0;
    let rect = ui.available_rect_before_wrap();
    let left_w = (rect.width() - gap) * (1.15 / 2.15);
    let left_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.min.x + left_w, rect.max.y));
    let right_rect =
        egui::Rect::from_min_max(egui::pos2(rect.min.x + left_w + gap, rect.min.y), rect.max);
    for (pane_rect, is_map) in [(left_rect, true), (right_rect, false)] {
        let mut pane = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(pane_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        pane.set_min_size(pane_rect.size());
        card_frame().show(&mut pane, |ui| {
            ui.set_min_size(ui.available_size());
            if is_map {
                crate::views::map::show(app, ui);
            } else {
                crate::views::bindings::show(app, ui);
            }
        });
    }
    ui.allocate_rect(rect, egui::Sense::hover());
}

fn show_header(
    ui: &mut egui::Ui,
    header: &HeaderState,
    title: &str,
    total_bindings: usize,
    palette: &Palette,
) -> ToolbarActions {
    let mut actions = ToolbarActions::default();

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("\u{2039} Library")
                        .font(theme::semibold(12.0))
                        .color(palette.ink_2),
                )
                .fill(egui::Color32::TRANSPARENT),
            )
            .clicked()
        {
            actions.go_back = true;
        }
        ui.add_space(6.0);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(egui::Label::new(theme::eyebrow(&header.breadcrumb)));
            ui.label(theme::display_title(title, 18.0));
        });

        ui.add_space(10.0);
        stat(ui, palette, total_bindings, "bindings");
        if header.dirty {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("unsaved")
                    .font(theme::mono_bold(12.0))
                    .color(palette.mouse),
            );
        }

        // Right-aligned toolbar: ghost buttons first (right-to-left order in
        // egui), then the primary "Save to QuadStick" at the far right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // No in-place target for a community source; in-flight saves gate
            // all save affordances so concurrent writes cannot race one target.
            let idle = !header.save_in_flight;
            let can_save_in_place = !header.is_community;

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Primary action — rendered rightmost (added first in r-t-l).
                if ui
                    .add_enabled(
                        header.volume_present && idle,
                        theme::primary_button("Save to QuadStick"),
                    )
                    .clicked()
                {
                    actions.save_to_qs = true;
                }
                if ui
                    .add_enabled(idle, egui::Button::new("Save As..."))
                    .clicked()
                {
                    actions.save_as = true;
                }
            }
            if ui
                .add_enabled(
                    can_save_in_place && header.dirty && idle,
                    egui::Button::new("Save"),
                )
                .clicked()
            {
                actions.save = true;
            }
            if ui.button("Preview CSV").clicked() {
                actions.preview = true;
            }
            if ui
                .add_enabled(header.can_redo, egui::Button::new("Redo"))
                .clicked()
            {
                actions.redo = true;
            }
            if ui
                .add_enabled(header.can_undo, egui::Button::new("Undo"))
                .clicked()
            {
                actions.undo = true;
            }
        });
    });
    actions
}

fn dispatch_toolbar(app: &mut YokeApp, actions: &ToolbarActions) {
    if actions.undo {
        app.undo_edit();
    }
    if actions.redo {
        app.redo_edit();
    }
    if actions.preview {
        app.open_preview();
    }
    if actions.save {
        app.save_in_place();
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if actions.save_as {
            app.save_as();
        }
        if actions.save_to_qs {
            app.save_to_device();
        }
    }
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
        // A horizontal scroll holding every chip on one line. Scrolling (not
        // wrapping) matches the design's single-row strip and keeps the
        // "+ Add layer" chip reachable at any layer count.
        egui::ScrollArea::horizontal()
            .id_salt("subprofile_strip")
            // Fill remaining width, but shrink vertically to the chip
            // height — otherwise the strip claims all leftover panel
            // height and leaves a tall empty band above the body.
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    show_chips(
                        ui,
                        palette,
                        tabs,
                        selected,
                        &ui_state,
                        &mut new_ui_state,
                        &mut new_selection,
                    );
                });
            });
    });

    // Selected-chip actions: a thin row directly below the strip frame so
    // rename/clone/delete stay visible regardless of horizontal scroll.
    if matches!(ui_state, SubProfileUi::Closed) {
        let selected_label = tabs.get(selected).map_or_else(
            || format!("Sub-profile {}", selected + 1),
            |(name, sublabel, _)| {
                if sublabel.is_empty() {
                    name.clone()
                } else {
                    format!("{name} · {sublabel}")
                }
            },
        );
        show_chip_buttons(
            app,
            ui,
            selected,
            &selected_label,
            palette,
            &mut action,
            &mut new_ui_state,
        );
    }

    // Inline forms — rendered below the strip/actions.
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

/// Rename/clone/delete are NOT in this row — they live in `show_chip_buttons`.
fn show_chips(
    ui: &mut egui::Ui,
    palette: &Palette,
    tabs: &[Tab],
    selected: usize,
    ui_state: &SubProfileUi,
    new_ui_state: &mut Option<SubProfileUi>,
    new_selection: &mut Option<usize>,
) {
    for (i, (name, _, count)) in tabs.iter().enumerate() {
        let is_selected = selected == i;
        if show_chip(ui, palette, i, name, *count, is_selected) && !is_selected {
            *new_selection = Some(i);
            // Closing any open inline form when selecting a different chip is
            // intentional: the form targets the previously selected index and
            // switching chip identity invalidates it.
            *new_ui_state = Some(SubProfileUi::Closed);
        }
    }

    if matches!(ui_state, SubProfileUi::Closed) {
        let add_resp = ui.add(
            egui::Button::new(
                egui::RichText::new("+ Add layer")
                    .font(theme::medium(12.5))
                    .color(palette.ink_3),
            )
            .frame(false),
        );
        if add_resp.clicked() {
            *new_ui_state = Some(SubProfileUi::Adding {
                name: String::new(),
                mode: 0,
                sub_mode: "Normal".into(),
                channel: 0,
            });
        }
    }
}

/// Render one sub-profile chip (design `.sub-tab`). Returns `true` when clicked
/// this frame.
fn show_chip(
    ui: &mut egui::Ui,
    palette: &Palette,
    index: usize,
    name: &str,
    count: usize,
    selected: bool,
) -> bool {
    let frame = theme::sub_tab_frame(selected);
    theme::clickable_frame(ui, frame, index, |ui| {
        ui.horizontal(|ui| {
            let name_color = if selected {
                palette.ink_1
            } else {
                palette.ink_2
            };
            ui.add(egui::Label::new(
                egui::RichText::new(name)
                    .font(theme::semibold(12.5))
                    .color(name_color),
            ));
            ui.label(
                egui::RichText::new(count.to_string())
                    .monospace()
                    .size(10.5)
                    .color(palette.ink_3),
            );
        });
    })
    .clicked()
}

/// Render the rename/clone/delete buttons for the currently-selected chip on a
/// thin row below the strip, prefixed with the selected layer's label. Kept out
/// of the scrollable chip row so the actions stay visible at any layer count.
/// The `index` is the currently-selected sub-profile.
#[allow(clippy::too_many_arguments)]
fn show_chip_buttons(
    app: &YokeApp,
    ui: &mut egui::Ui,
    index: usize,
    selected_label: &str,
    palette: &Palette,
    action: &mut Option<StripAction>,
    new_ui_state: &mut Option<SubProfileUi>,
) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("Selected: {selected_label}"))
                .small()
                .color(palette.ink_3),
        );
        if ui.add(theme::mini_button("rename")).clicked() {
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
        if ui.add(theme::mini_button("clone")).clicked() {
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
        if ui.add(theme::mini_button("delete")).clicked() {
            *action = Some(StripAction::Delete { index });
        }
    });
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
        if commit || ui.add(theme::mini_button("ok")).clicked() {
            *action = Some(StripAction::Rename {
                index,
                to: buf.clone(),
            });
            // set_subprofile_ui(Closed) is handled in dispatch_strip_action
        } else if ui.add(theme::mini_button("cancel")).clicked() {
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
        if ui.add(theme::mini_button("add")).clicked() {
            *action = Some(StripAction::Add {
                name: name_buf.clone(),
                mode: SubProfileMode::KNOWN[mode_idx].clone(),
                sub_mode: sub_mode_buf.clone(),
                channel: Channel::ALL[channel_idx],
            });
            // set_subprofile_ui(Closed) handled in dispatch_strip_action
        } else if ui.add(theme::mini_button("cancel")).clicked() {
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
                // A refused op keeps the form (and the typed fields) open so
                // the user can correct it instead of retyping.
                app.set_subprofile_ui(SubProfileUi::Closed);
            }
            app.report_edit(r);
        }
        Some(StripAction::Clone { index, to }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .clone_sub_profile(index, &to);
            if r.is_ok() {
                // Like Add: the clone lands at the tail; focus what was created.
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
        }
        Some(StripAction::Rename { index, to }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .rename_sub_profile(index, &to);
            if r.is_ok() {
                app.set_subprofile_ui(SubProfileUi::Closed);
            }
            app.report_edit(r);
        }
        Some(StripAction::Delete { index }) => {
            let r = app
                .edit_session_mut()
                .expect("open")
                .delete_sub_profile(index);
            if r.is_ok() {
                // Selection must never point past the shrunken strip.
                app.clamp_selected_subprofile();
            }
            // LastSubProfileDeletion surfaces as a toast; state is untouched.
            app.report_edit(r);
        }
        None => {}
    }
}

fn stat(ui: &mut egui::Ui, palette: &Palette, n: usize, label: &str) {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        ui.label(
            egui::RichText::new(n.to_string())
                .font(theme::mono_bold(12.0))
                .color(palette.ink_1),
        );
        ui.label(egui::RichText::new(label).size(12.0).color(palette.ink_2));
    });
}

// The selected-layer caption's dim sublabel. When the sub-profile carries an
// explicit `profile_name`, the name shows it and this returns the mode label.
// When `profile_name` is empty — the common case in real CSVs, where the name
// already shows the mode — this returns the `sub_mode` variant (e.g.
// "Alternate") only when it is meaningful, so the mode is never repeated and
// "Normal" is suppressed.
fn sub_sublabel(s: &SubProfile) -> String {
    let mode = s.header.mode.canonical_csv();
    if s.header.profile_name.trim().is_empty() {
        let sub_mode = s.header.sub_mode.trim();
        if sub_mode.is_empty() || sub_mode.eq_ignore_ascii_case("normal") {
            String::new()
        } else {
            sub_mode.to_owned()
        }
    } else if mode.trim().is_empty() {
        String::new()
    } else {
        mode
    }
}

// The sub-profile's base display name: the explicit profile name if present,
// else the mode (e.g. "Left Analog" / "Mixed joy"), falling back to an index
// only when neither carries a label. The sub-mode (Normal / Alternate / ...)
// is carried separately so the selected-layer caption can render it as a
// dim suffix.
fn sub_base_label(s: &SubProfile, i: usize) -> String {
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
