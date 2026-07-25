use crate::app::YokeApp;
use crate::data::{AppCommand, ProfileEntryView, ProfileKind};
use crate::state::CommunityLoad;
use crate::theme;

const CARD_HEIGHT: f32 = 112.0;
const CARD_ROW_GAP: f32 = 8.0;
const COLS: usize = 3;

/// Single source of truth for the device kind filter: maps each segment index
/// to its label and the kind it selects. Index 0 is "All" (no filter); both the
/// segmented control and the filter match read this so they can't desync.
const KIND_FILTERS: &[(&str, Option<ProfileKind>)] = &[
    ("All", None),
    ("Mouse + Keys", Some(ProfileKind::MouseKeys)),
    ("Gamepad", Some(ProfileKind::Gamepad)),
    ("Mixed", Some(ProfileKind::Mixed)),
];

fn kind_for_filter(index: usize) -> Option<ProfileKind> {
    KIND_FILTERS.get(index).and_then(|&(_, kind)| kind)
}

/// Cap on how many filtered community cards are rendered per frame.
/// Search filters the full list first; only the display slice is limited.
const LIB_COMMUNITY_DISPLAY_CAP: usize = 48;

/// Minimal card display data, decoupled from `ProfileName`'s native/wasm type.
#[derive(Clone, Copy)]
struct CardView<'a> {
    label: &'a str,
    kind: Option<ProfileKind>,
    sub_profiles: usize,
}

impl<'a> CardView<'a> {
    fn from_entry(e: &'a ProfileEntryView) -> Self {
        Self {
            label: &e.label,
            kind: e.kind,
            sub_profiles: e.sub_profiles,
        }
    }

    const fn community(label: &'a str) -> Self {
        Self {
            label,
            kind: None,
            sub_profiles: 0,
        }
    }
}

// uniform view signature; this view is read-only over app
#[allow(clippy::needless_pass_by_ref_mut)]
#[allow(clippy::too_many_lines)]
pub fn show(app: &mut YokeApp, ui: &mut egui::Ui) {
    let palette = *app.palette();

    // --- Header (pinned, not scrolled) ---
    ui.label(theme::display_title("Profiles", 24.0).extra_letter_spacing(-0.48));
    ui.horizontal(|ui| {
        let count = app.device_profiles().len();
        let status = app.device_status_text();
        let noun = if count == 1 { "profile" } else { "profiles" };
        ui.label(
            egui::RichText::new(format!("{count} {noun} · {status}"))
                .size(13.0)
                .color(palette.ink_2),
        );
        // Import only makes sense on native (no file dialog on wasm).
        #[cfg(not(target_arch = "wasm32"))]
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Import .csv").clicked() {
                app.open_file_dialog();
            }
        });
    });

    ui.add_space(14.0);

    // --- Toolbar (pinned, not scrolled) ---
    ui.horizontal(|ui| {
        theme::search_pill(ui, app.lib_search_mut(), "Search profiles");
        ui.add_space(2.0);
        let kind_labels: Vec<&str> = KIND_FILTERS.iter().map(|&(label, _)| label).collect();
        if let Some(i) = theme::segmented(ui, &kind_labels, app.lib_kind_filter()) {
            app.set_lib_kind_filter(i);
        }
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(10.0);

    // --- Collect filtered display data before any &mut app calls ---
    let search = app.lib_search().to_lowercase();
    let kind_filter = kind_for_filter(app.lib_kind_filter());

    let device_entries: Vec<_> = app
        .device_profiles()
        .iter()
        .filter(|e| {
            let text_match = search.is_empty() || e.label.to_lowercase().contains(&search);
            let kind_match = kind_filter.is_none_or(|k| e.kind == Some(k));
            text_match && kind_match
        })
        .cloned()
        .collect();

    let device_loading = app.device_loading();
    let device_total = app.device_profiles().len();

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
                if device_loading && device_total == 0 {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("Loading profiles from QuadStick\u{2026}")
                                .color(palette.ink_3),
                        );
                    });
                } else {
                    ui.label(egui::RichText::new("No profiles match.").color(palette.ink_3));
                }
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
                    let filtered: Vec<usize> = if kind_filter.is_some() {
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
            // The frame hugs content; force the full card rect (minus the
            // frame's 14px vertical margins) so footer-less cards match.
            ui.set_min_size(egui::vec2(ui.available_width(), CARD_HEIGHT - 28.0));

            ui.label(
                egui::RichText::new(card.label)
                    .font(theme::bold(15.0))
                    .color(palette.ink_1),
            );

            if card.kind.is_some() || card.sub_profiles > 1 {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.horizontal(|ui| {
                        if let Some(kind) = card.kind {
                            theme::kind_badge(ui, kind.label());
                        }
                        // Singular "1 layers" reads wrong, so the count shows
                        // only for multi-layer profiles.
                        if card.sub_profiles > 1 {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} layers",
                                            card.sub_profiles
                                        ))
                                        .monospace()
                                        .size(11.0)
                                        .color(palette.ink_3),
                                    );
                                },
                            );
                        }
                    });
                    ui.add_space(10.0);
                    let (line_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 1.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().hline(
                        line_rect.x_range(),
                        line_rect.center().y,
                        egui::Stroke::new(1.0, palette.line),
                    );
                });
            }
        });
    }

    response.clicked()
}

#[cfg(not(target_arch = "wasm32"))]
const fn community_name(entry: &yoke_index::IndexEntry) -> &str {
    entry.name.as_str()
}
#[cfg(target_arch = "wasm32")]
const fn community_name(entry: &crate::data::mock::MockCommunityEntry) -> &str {
    entry.name.as_str()
}
