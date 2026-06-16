use crate::app::YokeApp;
use crate::data::{AppCommand, ProfileEntryView, ProfileKind};
use crate::state::CommunityLoad;
use crate::theme;

const CARD_HEIGHT: f32 = 150.0;
const CARD_ROW_GAP: f32 = 8.0;
const COLS: usize = 3;

/// Cap on how many filtered community cards are rendered per frame.
/// Search filters the full list first; only the display slice is limited.
const LIB_COMMUNITY_DISPLAY_CAP: usize = 48;

/// Minimal card display data, decoupled from `ProfileName`'s native/wasm type.
#[derive(Clone, Copy)]
struct CardView<'a> {
    label: &'a str,
    kind: Option<ProfileKind>,
    bindings: usize,
    sub_profiles: usize,
    modes: &'a [String],
}

impl<'a> CardView<'a> {
    fn from_entry(e: &'a ProfileEntryView) -> Self {
        Self {
            label: &e.label,
            kind: e.kind,
            bindings: e.bindings,
            sub_profiles: e.sub_profiles,
            modes: &e.modes,
        }
    }

    const fn community(label: &'a str) -> Self {
        Self {
            label,
            kind: None,
            bindings: 0,
            sub_profiles: 0,
            modes: &[],
        }
    }
}

// uniform view signature; this view is read-only over app
#[allow(clippy::needless_pass_by_ref_mut)]
#[allow(clippy::too_many_lines)]
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let palette = *app.palette();

    // --- Header (pinned, not scrolled) ---
    ui.label(theme::eyebrow("Profile Library"));
    ui.add_space(4.0);
    ui.heading("Your profiles");
    ui.horizontal(|ui| {
        let count = app.device_profiles().len();
        let status = app.device_status_text();
        ui.label(
            egui::RichText::new(format!("{count} on QuadStick · {status}")).color(palette.ink_2),
        );
        // Import only makes sense on native (no file dialog on wasm).
        #[cfg(not(target_arch = "wasm32"))]
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Import .csv").clicked() {
                app.open_file_dialog();
            }
        });
    });

    ui.add_space(8.0);

    // --- Toolbar (pinned, not scrolled) ---
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(app.lib_search_mut())
                .desired_width(260.0)
                .hint_text("Search profiles\u{2026}"),
        );
        ui.add_space(8.0);
        if let Some(i) = theme::segmented(
            ui,
            &["All", "Mouse + Keys", "Gamepad", "Mixed"],
            app.lib_kind_filter(),
        ) {
            app.set_lib_kind_filter(i);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(theme::eyebrow("Sorted by name"));
        });
    });
    ui.separator();
    ui.add_space(8.0);

    // --- Collect filtered display data before any &mut app calls ---
    let search = app.lib_search().to_lowercase();
    let kind_filter = app.lib_kind_filter();

    let device_entries: Vec<_> = app
        .device_profiles()
        .iter()
        .filter(|e| {
            let text_match = search.is_empty() || e.label.to_lowercase().contains(&search);
            let kind_match = match kind_filter {
                1 => e.kind == Some(ProfileKind::MouseKeys),
                2 => e.kind == Some(ProfileKind::Gamepad),
                3 => e.kind == Some(ProfileKind::Mixed),
                _ => true,
            };
            text_match && kind_match
        })
        .cloned()
        .collect();

    // O(1) Arc refcount bump — no deep clone of the community list.
    let community = app.community().clone();

    // --- Single scrollable body: device grid + community grid ---
    // Open actions collected here; dispatched after the scroll area closes so
    // &mut app calls don't conflict with the closure borrow.
    let mut open_device: Option<(_, String)> = None;
    // Clicked community entry cloned from the owned snapshot; no re-borrow of app needed.
    let mut open_community_click: Option<(_, String)> = None;

    egui::ScrollArea::vertical()
        .id_salt("library_body")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ---- ON QUADSTICK section ----
            ui.label(theme::eyebrow("On QuadStick"));
            ui.add_space(4.0);

            if device_entries.is_empty() {
                ui.label(egui::RichText::new("No profiles match.").color(palette.ink_3));
            } else {
                for chunk in device_entries.chunks(COLS) {
                    // Build card views and check for clicks before calling open (which
                    // borrows app mutably and would conflict). Always split the row into
                    // COLS columns so a partial row's cards stay ~1/3 width (left-aligned,
                    // trailing columns empty) rather than stretching to the panel width.
                    let mut clicked_idx: Option<usize> = None;
                    ui.columns(COLS, |cols| {
                        for (col, entry) in chunk.iter().enumerate() {
                            if profile_card(&mut cols[col], &palette, &CardView::from_entry(entry))
                            {
                                clicked_idx = Some(col);
                            }
                        }
                    });
                    if let Some(col) = clicked_idx {
                        let e = &chunk[col];
                        open_device = Some((e.name.clone(), e.label.clone()));
                    }
                    ui.add_space(CARD_ROW_GAP);
                }
            }

            ui.add_space(12.0);

            // ---- COMMUNITY section ----
            ui.label(theme::eyebrow("Community"));
            ui.add_space(4.0);

            // Match on the owned Arc snapshot — no borrow of `app` held across
            // the deferred scroll closure.
            match &community {
                CommunityLoad::Loading => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading\u{2026}");
                    });
                }
                CommunityLoad::Loaded(entries) => {
                    // Community entries carry no kind; when a kind filter is active, show none.
                    let filtered: Vec<usize> = if kind_filter != 0 {
                        Vec::new()
                    } else if search.is_empty() {
                        (0..entries.len()).collect()
                    } else {
                        entries
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| community_name(e).to_lowercase().contains(&search))
                            .map(|(i, _)| i)
                            .collect()
                    };

                    let total = filtered.len();

                    if total == 0 {
                        ui.label(
                            egui::RichText::new("No community profiles match.")
                                .color(palette.ink_3),
                        );
                    } else {
                        let display = &filtered[..total.min(LIB_COMMUNITY_DISPLAY_CAP)];

                        for chunk in display.chunks(COLS) {
                            // Fixed COLS columns so a partial last row's cards stay
                            // ~1/3 width and left-aligned, matching the device grid.
                            let mut clicked_col: Option<usize> = None;
                            // Borrow one name per visible card; CardView holds &str, no clone.
                            let names: Vec<&str> =
                                chunk.iter().map(|&i| community_name(&entries[i])).collect();
                            ui.columns(COLS, |cols| {
                                for (col, name) in names.iter().copied().enumerate() {
                                    if profile_card(
                                        &mut cols[col],
                                        &palette,
                                        &CardView::community(name),
                                    ) {
                                        clicked_col = Some(col);
                                    }
                                }
                            });
                            if let Some(col) = clicked_col {
                                // Clone exactly the clicked entry from the owned snapshot.
                                let idx = chunk[col];
                                let entry = entries[idx].clone();
                                let name = community_name(&entries[idx]).to_string();
                                open_community_click = Some((entry, name));
                            }
                            ui.add_space(CARD_ROW_GAP);
                        }

                        if total > LIB_COMMUNITY_DISPLAY_CAP {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Showing {LIB_COMMUNITY_DISPLAY_CAP} of {total} community profiles \u{2014} refine search to narrow."
                                ))
                                .small()
                                .color(palette.ink_3),
                            );
                        }
                    }
                }
                CommunityLoad::Failed(msg) => {
                    ui.colored_label(palette.system, format!("Failed: {msg}"));
                    if ui.button("Retry").clicked() {
                        app.send(AppCommand::ListCommunity);
                    }
                }
                CommunityLoad::Disabled => {
                    ui.label(
                        egui::RichText::new("Community profiles unavailable.")
                            .color(palette.ink_3),
                    );
                }
            }
        });

    // Dispatch open actions after the scroll area closes (avoids &mut borrow
    // conflict inside the closure).
    if let Some((name, label)) = open_device {
        app.open_device_profile(name, label);
    } else if let Some((entry, name)) = open_community_click {
        app.open_community(entry, name);
    }
}

/// Render a single profile card using `card_frame`. Returns true if clicked.
fn profile_card(ui: &mut egui::Ui, palette: &crate::theme::Palette, card: &CardView<'_>) -> bool {
    let desired_size = egui::vec2(ui.available_width(), CARD_HEIGHT);
    // Allocate a fixed rect first; the actual card frame is drawn inside it.
    // `Sense::click` on the allocated response makes the whole area clickable.
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        theme::card_frame().show(&mut child, |ui| {
            // Top row: kind badge left, filename right. The muted filename is
            // shown only for kinded (device) cards; community cards have no kind
            // and would duplicate their mono-bold title here.
            if let Some(kind) = card.kind {
                ui.horizontal(|ui| {
                    let color = kind_color(palette, kind);
                    theme::kind_badge(ui, kind.label(), color);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(
                            egui::RichText::new(card.label)
                                .monospace()
                                .size(11.0)
                                .color(palette.ink_3),
                        );
                    });
                });
            }

            ui.add_space(6.0);

            // Title is the filename in monospace bold, matching the design.
            ui.label(
                egui::RichText::new(card.label)
                    .monospace()
                    .strong()
                    .size(15.0),
            );

            ui.add_space(6.0);

            // Mode chips (up to 3).
            if !card.modes.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    for mode in card.modes.iter().take(3) {
                        theme::pill_frame().show(ui, |ui| {
                            ui.label(egui::RichText::new(mode).monospace().size(10.5));
                        });
                    }
                });
                ui.add_space(2.0);
            }

            // Binding / sub-profile count.
            if card.bindings > 0 || card.sub_profiles > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{} bindings \u{00B7} {} sub-profiles",
                        card.bindings, card.sub_profiles
                    ))
                    .size(11.0)
                    .color(palette.ink_3),
                );
            }
        });
    }

    response.clicked()
}

/// Map a `ProfileKind` to the palette color for its badge.
const fn kind_color(palette: &crate::theme::Palette, kind: ProfileKind) -> egui::Color32 {
    match kind {
        ProfileKind::MouseKeys => palette.keyboard,
        ProfileKind::Gamepad => palette.joystick,
        ProfileKind::Mixed => palette.accent,
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn community_name(entry: &yoke_index::IndexEntry) -> &str {
    entry.name.as_str()
}
#[cfg(target_arch = "wasm32")]
const fn community_name(entry: &crate::data::mock::MockCommunityEntry) -> &str {
    entry.name.as_str()
}
