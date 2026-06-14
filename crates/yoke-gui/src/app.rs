use crate::data::{AppCommand, DataEvent, FailureContext};
use crate::state::{CommunityLoad, OpenProfile, ProfileSource};
use crate::theme::{self, Palette};

#[cfg(target_arch = "wasm32")]
use crate::data::mock::{MockCommunityEntry as IndexEntry, MockMountState as MountState};
#[cfg(not(target_arch = "wasm32"))]
use yoke_volume::state::{ModeHint, MountState};
#[cfg(not(target_arch = "wasm32"))]
use {yoke_index::IndexEntry, yoke_volume::ProfileName};

#[cfg(target_arch = "wasm32")]
type ProfileName = String;

/// State of the sub-profile management UI in the chip strip.
#[derive(Debug, Clone)]
pub enum SubProfileUi {
    Closed,
    Renaming {
        index: usize,
        value: String,
    },
    Adding {
        name: String,
        /// Index into `SubProfileMode::KNOWN`.
        mode: usize,
        sub_mode: String,
        /// Index into `Channel::ALL`.
        channel: usize,
    },
}

/// What the open picker edits. Captured at open time, including the
/// sub-profile index, so a row is always addressed by the state the user saw.
#[derive(Debug, Clone)]
pub enum PickerTarget {
    AddBinding {
        input: String,
    },
    EditOutput {
        input: String,
        modifier: String,
    },
    EditModifier {
        input: String,
        output: String,
        modifier: String,
    },
}

#[derive(Debug, Clone)]
pub struct PickerState {
    pub sub: usize,
    pub target: PickerTarget,
    pub search: String,
    pub category: Option<&'static str>,
    pub capture_armed: bool,
    pub capture_error: Option<String>,
    pub keyword: String,
    pub args: Vec<String>,
}

impl PickerState {
    fn new(sub: usize, target: PickerTarget) -> Self {
        let (keyword, args) = match &target {
            PickerTarget::EditModifier { modifier, .. } => seed_modifier_fields(modifier),
            _ => ("normal".to_owned(), Vec::new()),
        };
        Self {
            sub,
            target,
            search: String::new(),
            category: None,
            capture_armed: false,
            capture_error: None,
            keyword,
            args,
        }
    }
}

/// Split an existing modifier csv (`"delay_on 1000"`) into keyword + padded
/// argument fields so the editor opens pre-filled.
fn seed_modifier_fields(modifier: &str) -> (String, Vec<String>) {
    let mut tokens = modifier.split_whitespace();
    let keyword = tokens.next().unwrap_or("normal").to_owned();
    let labels = crate::edit::modifier_arg_labels(&keyword);
    let mut args: Vec<String> = tokens.map(ToOwned::to_owned).collect();
    args.resize(labels.len(), String::new());
    (keyword, args)
}

/// In-flight profile open. `req` is the monotonic id stamped when the open was
/// dispatched; events carry it back so a stale result (a slower open finishing
/// after a newer one, or after the user backed out) can be dropped.
struct OpenInFlight {
    req: u64,
    label: String,
}

/// What a `VolumeChanged` event implies for the cached device-profile list.
enum VolumeAction {
    /// The readable volume appeared or the mounted device changed: (re)list.
    Relist,
    /// The volume went away: drop the stale list.
    Clear,
    Nothing,
}

#[cfg(not(target_arch = "wasm32"))]
fn volume_action(old: Option<&MountState>, new: &MountState) -> VolumeAction {
    let mount_of = |s: &MountState| match s {
        MountState::Present { mount_point, .. } => Some(mount_point.clone()),
        _ => None,
    };
    // Compare the mount point, not just presence, so a hot-swap that publishes
    // Present(A) -> Present(B) re-lists against the new device instead of
    // serving the previous device's stale entries.
    match (old.and_then(&mount_of), mount_of(new)) {
        (was, Some(now)) if was.as_ref() != Some(&now) => VolumeAction::Relist,
        (Some(_), None) => VolumeAction::Clear,
        _ => VolumeAction::Nothing,
    }
}
#[cfg(target_arch = "wasm32")]
fn volume_action(old: Option<&MountState>, new: &MountState) -> VolumeAction {
    match (
        old.is_some_and(|s| matches!(s, MountState::Present)),
        matches!(new, MountState::Present),
    ) {
        (false, true) => VolumeAction::Relist,
        (true, false) => VolumeAction::Clear,
        _ => VolumeAction::Nothing,
    }
}

/// Failure contexts tied to an in-flight open (reconciled against the latest
/// open request); list-style failures are not.
const fn is_open_context(c: FailureContext) -> bool {
    matches!(
        c,
        FailureContext::OpenDevice | FailureContext::OpenFile | FailureContext::OpenCommunity
    )
}

// Each bool is an independent UI/lifecycle flag (community gating, startup
// latch, discard prompt, device-list loading); folding them into one state
// machine would couple unrelated concerns.
#[allow(clippy::struct_excessive_bools)]
pub struct YokeApp {
    palette: Palette,
    worker: crate::worker::WorkerHandle,
    #[cfg(not(target_arch = "wasm32"))]
    events: std::sync::mpsc::Receiver<DataEvent>,

    volume: Option<MountState>,
    backend_error: Option<String>,
    device_profiles: Vec<crate::data::ProfileEntryView>,
    /// True from the list dispatch until the matching `ProfilesListed` (or a
    /// list failure) settles; drives the library spinner so the window stays
    /// live while the slow FAT/USB volume is read.
    device_loading: bool,
    community: CommunityLoad,
    open_profile: Option<OpenProfile>,
    selected_station: Option<&'static str>,
    selected_subprofile: usize,
    subprofile_ui: SubProfileUi,
    toast: Option<(String, f64)>,
    picker: Option<PickerState>,
    /// The profile open currently in flight (read/download + parse on the
    /// worker); drives the loading overlay. Cleared on the matching
    /// `ProfileOpened`/`FileDialogCancelled`/failure, or when the user backs out.
    opening: Option<OpenInFlight>,
    /// Monotonic source of `OpenInFlight::req` ids.
    next_req: u64,
    /// Whether the community index is usable; gates the initial `ListCommunity`.
    community_available: bool,
    requested_initial: bool,
    /// Whether the confirm-discard modal is showing. Set when the user requests
    /// a close on a dirty session; the modal forces an explicit choice before
    /// any data is discarded. Silently discarding edits is a critical bug class.
    pub(crate) confirm_discard: bool,
    /// Library search text; filters both device and community card grids.
    lib_search: String,
    /// Library kind-filter index: 0 = All, 1 = `MouseKeys`, 2 = Gamepad, 3 = Mixed.
    lib_kind_filter: usize,
    /// A save that has been dispatched but not yet confirmed: `(req_id, snapshot_at_dispatch)`.
    /// The snapshot is the profile state serialized at dispatch time — edits made while
    /// the save is in-flight must stay dirty after the save completes.
    pending_save: Option<(u64, yoke_config::model::Profile)>,
    /// CSV text for the preview modal. `Some` while the modal is open.
    preview_csv: Option<String>,
}

impl YokeApp {
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    // Receiver and WorkerHandle are not const-constructible.
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(
        worker: crate::worker::WorkerHandle,
        events: std::sync::mpsc::Receiver<DataEvent>,
        backend_error: Option<String>,
        community_available: bool,
    ) -> Self {
        Self {
            palette: Palette::console(),
            worker,
            events,
            volume: None,
            backend_error,
            device_profiles: Vec::new(),
            device_loading: false,
            community: CommunityLoad::Loading,
            open_profile: None,
            selected_station: None,
            selected_subprofile: 0,
            subprofile_ui: SubProfileUi::Closed,
            toast: None,
            picker: None,
            opening: None,
            next_req: 0,
            community_available,
            requested_initial: false,
            confirm_discard: false,
            pending_save: None,
            preview_csv: None,
            lib_search: String::new(),
            lib_kind_filter: 0,
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[must_use]
    // WorkerHandle is not const-constructible.
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(worker: crate::worker::WorkerHandle, community_available: bool) -> Self {
        Self {
            palette: Palette::console(),
            worker,
            volume: None,
            backend_error: None,
            device_profiles: Vec::new(),
            device_loading: false,
            community: CommunityLoad::Loading,
            open_profile: None,
            selected_station: None,
            selected_subprofile: 0,
            subprofile_ui: SubProfileUi::Closed,
            toast: None,
            picker: None,
            opening: None,
            next_req: 0,
            community_available,
            requested_initial: false,
            confirm_discard: false,
            pending_save: None,
            preview_csv: None,
            lib_search: String::new(),
            lib_kind_filter: 0,
        }
    }

    fn drain_events(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let drained: Vec<DataEvent> = self.events.try_iter().collect();
        #[cfg(target_arch = "wasm32")]
        let drained: Vec<DataEvent> = self.worker.drain();
        for ev in drained {
            self.apply_event(ev);
        }
    }

    /// Mark the device list as loading and dispatch a fresh list; drives the
    /// library spinner so the window stays live while the volume is read.
    fn dispatch_list_device(&mut self) {
        self.device_loading = true;
        self.worker.send(AppCommand::ListDeviceProfiles);
    }

    fn apply_event(&mut self, ev: DataEvent) {
        match ev {
            DataEvent::ProfilesListed(list) => {
                self.device_profiles = list;
                self.device_loading = false;
            }
            DataEvent::CommunityListed(list) => {
                self.community = CommunityLoad::Loaded(std::sync::Arc::new(list));
            }
            DataEvent::VolumeChanged(state) => {
                // The volume watcher fires this on mount/unmount, so the device
                // list tracks the device live.
                match volume_action(self.volume.as_ref(), &state) {
                    VolumeAction::Relist => self.dispatch_list_device(),
                    VolumeAction::Clear => {
                        self.device_profiles.clear();
                        self.device_loading = false;
                    }
                    VolumeAction::Nothing => {}
                }
                self.volume = Some(state);
            }
            DataEvent::ProfileOpened {
                req,
                source,
                parsed,
            } => {
                // Drop a stale open: a newer open superseded this request, or the
                // user backed out. Without this, a slow open finishing after a
                // faster later one would clobber the editor.
                if self.opening.as_ref().map(|o| o.req) != Some(req) {
                    return;
                }
                self.opening = None;
                self.selected_station = None;
                self.selected_subprofile = 0;
                self.subprofile_ui = SubProfileUi::Closed;
                // A stale discard prompt must not appear over a newly opened profile.
                self.confirm_discard = false;
                self.open_profile = Some(OpenProfile {
                    source,
                    session: crate::edit::EditSession::new(*parsed),
                });
            }
            DataEvent::FileDialogCancelled { req } => {
                if self.opening.as_ref().map(|o| o.req) == Some(req) {
                    self.opening = None;
                }
                // Save As cancelled: drop the pending save so a later Saved
                // event (from a different dialog run) can't ghostly mark it clean.
                if self.pending_save.as_ref().map(|(r, _)| *r) == Some(req) {
                    self.pending_save = None;
                }
            }
            DataEvent::Saved { req, label } => {
                // Edits made while the save was in flight stay dirty: mark_saved
                // gets the snapshot serialized at dispatch time, not current().
                if self.pending_save.as_ref().map(|(r, _)| *r) == Some(req) {
                    let (_, snapshot) = self.pending_save.take().expect("matched above");
                    if let Some(open) = &mut self.open_profile {
                        open.session.mark_saved(snapshot);
                    }
                    self.set_toast(format!("Saved to {label}"));
                }
            }
            DataEvent::Failed {
                req,
                context,
                message,
            } => self.handle_failure(req, context, message),
        }
    }

    fn handle_failure(&mut self, req: Option<u64>, context: FailureContext, message: String) {
        // Save failures reconcile against pending_save: a stale failure (from a
        // superseded save, or a None req that can never match) is dropped silently.
        // A None req must never match a pending save — only Some(r) == Some(r) matches.
        if matches!(
            context,
            FailureContext::SaveFile | FailureContext::SaveDevice
        ) {
            let pending = self.pending_save.as_ref().map(|(r, _)| *r);
            if pending.is_none() || pending != req {
                return;
            }
            self.pending_save = None;
            self.set_toast(message);
            return;
        }

        // Open-style failures reconcile against the in-flight request: a stale
        // failure (superseded by a newer open, or arriving after the user backed
        // out) is dropped so it can't clear a newer open's spinner or toast.
        if is_open_context(context) {
            if self.opening.as_ref().map(|o| o.req) != req {
                return;
            }
            self.opening = None;
        }
        match context {
            // ListDevice: the empty Library + "Disconnected" pill already convey
            // "no device"; a red toast on every device-less cold start is noise.
            // The list never settled, so the spinner must stop.
            FailureContext::ListDevice => self.device_loading = false,
            // SaveFile/SaveDevice: handled in the save branch above; unreachable here.
            FailureContext::SaveFile | FailureContext::SaveDevice => {}
            FailureContext::ListCommunity => self.community = CommunityLoad::Failed(message),
            // A single failed entry-open must not wipe the list the user is
            // browsing; the list-wide Failed state is reachable only from
            // ListCommunity. Surface the per-entry failure as a toast.
            FailureContext::OpenCommunity => {
                if !matches!(self.community, CommunityLoad::Loaded(_)) {
                    self.community = CommunityLoad::Failed(message.clone());
                }
                self.set_toast(message);
            }
            FailureContext::OpenDevice | FailureContext::OpenFile => self.set_toast(message),
        }
    }

    pub(crate) fn set_toast(&mut self, message: String) {
        // f64::MAX as "unset" sentinel; expiry is written on first paint in show_toast.
        self.toast = Some((message, f64::MAX));
    }
}

impl eframe::App for YokeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_events();

        if !self.requested_initial {
            self.requested_initial = true;
            self.dispatch_list_device();
            if self.community_available {
                self.worker.send(AppCommand::ListCommunity);
            } else {
                // Unavailable index never recovers at runtime; render it disabled
                // rather than firing a list that fails into a retry-forever state.
                self.community = CommunityLoad::Disabled;
            }
        }

        let ctx = ui.ctx().clone();
        let style = ctx.global_style();
        let top_frame =
            egui::Frame::side_top_panel(&style).inner_margin(egui::Margin::symmetric(16, 12));
        let rail_frame =
            egui::Frame::side_top_panel(&style).inner_margin(egui::Margin::symmetric(12, 14));
        let central_frame =
            egui::Frame::central_panel(&style).inner_margin(egui::Margin::symmetric(24, 20));

        egui::Panel::top("yoke_top")
            .frame(top_frame)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Yoke");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.status_pill(ui);
                    });
                });
            });

        egui::Panel::left("yoke_rail")
            .resizable(false)
            // exact_size, not default_size: a non-resizable Panel persists its
            // rendered rect by id every frame and reads it back, so default_size
            // only seeds frame 1 and the rail then collapses to its content
            // width. exact_size pins the width every frame, wide enough that the
            // longest status label ("Connected - mass storage off") never wraps.
            .exact_size(260.0)
            .frame(rail_frame)
            .show_inside(ui, |ui| {
                let on_library = self.open_profile.is_none();
                if ui.selectable_label(on_library, "Profiles").clicked() {
                    self.request_close_profile();
                }
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("DEVICE")
                        .small()
                        .color(self.palette.ink_3),
                );
                ui.add_space(2.0);
                self.rail_device_status(ui);
            });

        let status_frame = theme::status_bar_frame().inner_margin(egui::Margin::symmetric(12, 0));

        egui::Panel::bottom("yoke_status")
            .frame(status_frame)
            .exact_size(28.0)
            .show_inside(ui, |ui| {
                self.status_bar(ui);
            });

        egui::CentralPanel::default()
            .frame(central_frame)
            .show_inside(ui, |ui| {
                if self.open_profile.is_some() {
                    crate::views::editor::show(self, ui);
                } else {
                    crate::views::library::show(self, ui);
                }
            });

        self.show_loading_overlay(&ctx);
        self.show_toast(&ctx, ui);
        self.show_preview(&ctx);

        // Read capture-armed before rendering: while capture is armed, an Escape
        // press is the user's chosen key and is consumed by the picker, so the
        // Escape chain below must not also act on it.
        let capture_was_armed = self.picker.as_ref().is_some_and(|p| p.capture_armed);
        self.show_picker(&ctx);

        self.show_confirm_discard(&ctx);
        self.handle_undo_redo_shortcuts(&ctx);

        // Escape steps back: picker -> confirm_discard prompt -> sub-profile form
        // -> station selection -> open profile (via request_close_profile, which
        // may raise the confirm prompt again for dirty sessions) -> pending open.
        // The sub-profile-form step comes before close so an Escape mid-rename/add
        // cancels the form instead of discarding the open profile.
        // Skip the whole chain on a capture-armed frame: that Escape was the
        // captured key, not a back-out, and acting on it here would deselect
        // the station underneath the picker.
        if !capture_was_armed && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.picker.is_some() {
                // Picker is open but modal did NOT consume Escape (e.g. a popup was
                // open inside the modal). Close the picker without falling through.
                self.picker = None;
            } else if self.preview_csv.is_some() {
                // The preview modal usually consumes Escape itself; this step is
                // the fallback so Escape can never fall through to deeper steps.
                self.preview_csv = None;
            } else if self.confirm_discard {
                // Dismiss the confirm prompt; the profile remains open.
                self.confirm_discard = false;
            } else if !matches!(self.subprofile_ui, SubProfileUi::Closed) {
                self.subprofile_ui = SubProfileUi::Closed;
            } else if self.selected_station.is_some() {
                self.selected_station = None;
            } else if self.open_profile.is_some() {
                self.request_close_profile();
            } else {
                self.opening = None;
            }
        }
    }
}

impl YokeApp {
    fn status_pill(&self, ui: &mut egui::Ui) {
        let (text, color) = self.status_label();
        ui.colored_label(color, text);
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let (status_text, dot_color) = self.status_label();

            // Paint a 6px filled circle as the sync-state indicator.
            let dot_size = egui::vec2(8.0, 8.0);
            let (dot_rect, _) = ui.allocate_exact_size(dot_size, egui::Sense::hover());
            ui.painter()
                .circle_filled(dot_rect.center(), 3.0, dot_color);

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(status_text)
                    .monospace()
                    .size(11.0)
                    .color(self.palette.ink_2),
            );

            // Native only: show the mount path when the volume is present.
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(MountState::Present { mount_point, .. }) = &self.volume {
                ui.label(
                    egui::RichText::new(format!("  \u{00B7}  {}", mount_point.display()))
                        .monospace()
                        .size(11.0)
                        .color(self.palette.ink_3),
                );
            }

            let community_count = if let CommunityLoad::Loaded(v) = &self.community {
                v.len()
            } else {
                0
            };
            let counts = format!(
                "  \u{00B7}  {} profiles  \u{00B7}  {} community",
                self.device_profiles.len(),
                community_count,
            );
            ui.label(
                egui::RichText::new(counts)
                    .monospace()
                    .size(11.0)
                    .color(self.palette.ink_3),
            );
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    const fn status_label(&self) -> (&'static str, egui::Color32) {
        if self.backend_error.is_some() {
            return ("backend error", self.palette.system);
        }
        match &self.volume {
            Some(MountState::Present { .. }) => ("Connected", self.palette.accent),
            // A QuadStick disk has appeared but its FAT filesystem is not yet
            // readable; say "Connecting" rather than guessing "mass storage off".
            Some(MountState::Mounting { .. }) => ("Connecting…", self.palette.mouse),
            // Device is plugged in but exposes no readable FAT volume: surface
            // why (mass-storage off / controller emulation) rather than calling
            // it disconnected. Amber distinguishes it from both other states.
            Some(MountState::DeviceVisibleNoVolume { mode_hint, .. }) => {
                let label = match mode_hint {
                    Some(ModeHint::MassStorageDisabled) => "Connected - mass storage off",
                    Some(ModeHint::Emulation) => "Connected - emulation mode",
                    Some(ModeHint::Ps4OrHori) => "Connected - controller mode",
                    None => "Connected - no volume",
                };
                (label, self.palette.mouse)
            }
            Some(MountState::Absent) | None => ("Disconnected", self.palette.ink_3),
        }
    }

    #[cfg(target_arch = "wasm32")]
    const fn status_label(&self) -> (&'static str, egui::Color32) {
        match &self.volume {
            Some(MountState::Present) => ("Connected", self.palette.accent),
            None => ("Disconnected", self.palette.ink_3),
        }
    }

    fn rail_device_status(&self, ui: &mut egui::Ui) {
        let (text, color) = self.status_label();
        ui.label(egui::RichText::new(text).color(color))
            .on_hover_text(text);
        if let Some(err) = &self.backend_error {
            ui.label(egui::RichText::new(err).small().color(self.palette.ink_3))
                .on_hover_text(
                    "Volume backend failed to initialize; file-open and community still work.",
                );
        }
    }

    #[allow(clippy::float_cmp)] // f64::MAX is a sentinel meaning "not yet set"; exact equality is intentional
    fn show_toast(&mut self, ctx: &egui::Context, _ui: &egui::Ui) {
        let now = ctx.input(|i| i.time);
        let Some((msg, expiry)) = self.toast.as_mut() else {
            return;
        };
        if *expiry == f64::MAX {
            *expiry = now + 5.0;
        }
        if now >= *expiry {
            self.toast = None;
            return;
        }
        let msg = msg.clone();
        egui::Area::new(egui::Id::new("yoke_toast"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.colored_label(self.palette.system, msg);
                });
            });
        ctx.request_repaint(); // keep ticking until dismissed
    }

    // Centered spinner while a profile open is in flight on the worker. The
    // spinner self-requests repaint, so the UI keeps ticking until the matching
    // event clears `opening`.
    fn show_loading_overlay(&self, ctx: &egui::Context) {
        let Some(open) = &self.opening else {
            return;
        };
        let label = open.label.clone();
        egui::Area::new(egui::Id::new("yoke_loading"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.add_space(6.0);
                            ui.label(format!("Loading {label}…"));
                        });
                    });
            });
    }

    /// Show the confirm-discard modal when `confirm_discard` is set. Forces an
    /// explicit choice before any edit is thrown away; silently discarding edits
    /// is a critical bug class for a device where the profile is the sole input path.
    ///
    /// Dismissal (Keep editing) is the safe default, so backdrop-click and
    /// Escape — surfaced together by `ModalResponse::should_close` — only ever
    /// keep the profile open, never discard. Escape is also handled by the global
    /// chain in `ui`; both paths just clear `confirm_discard`, so the duplicate is
    /// idempotent. `should_close` additionally covers the backdrop-click case the
    /// global chain does not see.
    fn show_confirm_discard(&mut self, ctx: &egui::Context) {
        if self.confirm_discard {
            let response = egui::Modal::new(egui::Id::new("yoke_discard")).show(ctx, |ui| {
                ui.heading("Discard unsaved changes?");
                ui.horizontal(|ui| {
                    if ui.button("Keep editing").clicked() {
                        self.confirm_discard = false;
                    }
                    if ui.button("Discard").clicked() {
                        self.confirm_discard = false;
                        self.close_profile();
                    }
                });
            });
            if response.should_close() {
                self.confirm_discard = false;
            }
        }
    }

    /// CSV preview modal: the exact bytes a save would write, so the user can
    /// inspect before committing. Save delegates to `save_in_place`.
    fn show_preview(&mut self, ctx: &egui::Context) {
        let Some(preview) = self.preview_csv.clone() else {
            return;
        };
        let can_save = self
            .open_profile
            .as_ref()
            .is_some_and(|o| !matches!(o.source, ProfileSource::Community { .. }))
            && self.pending_save.is_none();
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("yoke_preview")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading("CSV preview");
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&preview).monospace().size(11.0));
                });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_save, egui::Button::new("Save"))
                    .clicked()
                {
                    close = true;
                    self.save_in_place();
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.preview_csv = None;
        }
    }

    /// Handle Cmd+Z / Cmd+Shift+Z for profile-level undo/redo.
    ///
    /// Gated so the shortcut only fires when the profile editor owns the
    /// keyboard:
    /// - `picker.is_none()`: the picker is a modal handling its own input.
    /// - `!confirm_discard`: `egui::Modal` blocks pointers but NOT the keyboard,
    ///   so without this an undo behind the "Discard?" prompt would mutate the
    ///   session and make the prompt's premise stale (undone to clean while the
    ///   user reads the warning).
    /// - `subprofile_ui == Closed`: when a rename/add `TextEdit` is focused it
    ///   runs its own Cmd+Z undo (egui 0.34 `TextEdit` owns an undoer and reads
    ///   key events via the non-consuming `filtered_events`), so one press would
    ///   double-act — undoing the field AND a profile op. For a config that is
    ///   the user's sole input path, that silent extra mutation is unacceptable,
    ///   so Cmd+Z is ceded to the focused field.
    fn handle_undo_redo_shortcuts(&mut self, ctx: &egui::Context) {
        if !self.undo_redo_shortcuts_enabled() {
            return;
        }
        let (undo, redo) = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
                cmd && i.modifiers.shift && i.key_pressed(egui::Key::Z),
            )
        });
        if undo {
            self.undo_edit();
        } else if redo {
            self.redo_edit();
        }
    }

    /// The keyboard-ownership gate for the undo/redo shortcut; see
    /// [`Self::handle_undo_redo_shortcuts`] for why each condition is required.
    /// `preview_csv` is included because an undo behind the preview modal would
    /// silently mutate the session while the preview text stays stale.
    const fn undo_redo_shortcuts_enabled(&self) -> bool {
        self.picker.is_none()
            && !self.confirm_discard
            && self.preview_csv.is_none()
            && matches!(self.subprofile_ui, SubProfileUi::Closed)
    }

    const fn alloc_req(&mut self) -> u64 {
        let req = self.next_req;
        self.next_req = self.next_req.wrapping_add(1);
        req
    }

    // Dispatch an open with a fresh request id and show its loading overlay. The
    // id is echoed back on the resulting event so a stale result is dropped.
    pub(crate) fn open_device_profile(&mut self, name: ProfileName, label: impl Into<String>) {
        let req = self.alloc_req();
        self.opening = Some(OpenInFlight {
            req,
            label: label.into(),
        });
        self.worker
            .send(AppCommand::OpenDeviceProfile { req, name });
    }

    pub(crate) fn open_community(&mut self, entry: IndexEntry, label: impl Into<String>) {
        let req = self.alloc_req();
        self.opening = Some(OpenInFlight {
            req,
            label: label.into(),
        });
        self.worker.send(AppCommand::OpenCommunity { req, entry });
    }

    // Native-only: the browser build gates out the file-open button.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn open_file_dialog(&mut self) {
        let req = self.alloc_req();
        self.opening = Some(OpenInFlight {
            req,
            label: "Picking file".into(),
        });
        self.worker.send(AppCommand::OpenFileDialog { req });
    }

    pub(crate) const fn palette(&self) -> &Palette {
        &self.palette
    }
    pub(crate) fn device_profiles(&self) -> &[crate::data::ProfileEntryView] {
        &self.device_profiles
    }
    pub(crate) const fn device_loading(&self) -> bool {
        self.device_loading
    }
    pub(crate) const fn community(&self) -> &CommunityLoad {
        &self.community
    }
    pub(crate) const fn open_profile(&self) -> Option<&OpenProfile> {
        self.open_profile.as_ref()
    }
    /// Whether a dispatched save has not yet been confirmed; gates the save
    /// affordances so concurrent writes to one target cannot race.
    pub(crate) const fn save_in_flight(&self) -> bool {
        self.pending_save.is_some()
    }

    /// Whether the `QuadStick` FAT volume is mounted and writable, gating the
    /// "Save to `QuadStick`" affordance.
    pub(crate) const fn volume_present(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            matches!(self.volume, Some(MountState::Present { .. }))
        }
        #[cfg(target_arch = "wasm32")]
        {
            matches!(self.volume, Some(MountState::Present))
        }
    }
    pub(crate) const fn selected_station(&self) -> Option<&'static str> {
        self.selected_station
    }
    pub(crate) const fn set_selected_station(&mut self, s: Option<&'static str>) {
        self.selected_station = s;
    }
    pub(crate) const fn selected_subprofile(&self) -> usize {
        self.selected_subprofile
    }
    pub(crate) const fn set_selected_subprofile(&mut self, i: usize) {
        self.selected_subprofile = i;
    }
    pub(crate) fn close_profile(&mut self) {
        self.open_profile = None;
        self.selected_station = None;
        // A picker is always over the open profile; drop it together.
        self.picker = None;
        self.subprofile_ui = SubProfileUi::Closed;
        // Backing out also cancels a pending open so its loading overlay stops
        // painting over the Library; the in-flight result is dropped on arrival.
        self.opening = None;
        self.confirm_discard = false;
        // A Saved event arriving after the profile is closed must not toast
        // "Saved to …" or call mark_saved on a gone session; drop the pending save.
        self.pending_save = None;
        self.preview_csv = None;
    }

    /// Close the profile, but if the session is dirty, show the confirm-discard
    /// modal instead. Every user-intent close path must go through this; only
    /// the Discard button itself calls `close_profile` directly.
    pub(crate) fn request_close_profile(&mut self) {
        if self
            .open_profile
            .as_ref()
            .is_some_and(|o| o.session.is_dirty())
        {
            self.confirm_discard = true;
        } else {
            self.close_profile();
        }
    }
    pub(crate) const fn lib_search_mut(&mut self) -> &mut String {
        &mut self.lib_search
    }
    pub(crate) const fn lib_search(&self) -> &String {
        &self.lib_search
    }
    pub(crate) const fn lib_kind_filter(&self) -> usize {
        self.lib_kind_filter
    }
    pub(crate) const fn set_lib_kind_filter(&mut self, i: usize) {
        self.lib_kind_filter = i;
    }
    pub(crate) const fn device_status_text(&self) -> &'static str {
        self.status_label().0
    }

    pub(crate) fn send(&self, cmd: AppCommand) {
        self.worker.send(cmd);
    }

    /// Serialize the current session, stamp a pending-save slot, and dispatch
    /// `cmd_for(req, bytes)` to the worker. Snapshot semantics: in-flight edits
    /// stay dirty after the save because `mark_saved` receives the profile as it
    /// was at dispatch time, not at completion time.
    fn dispatch_save(&mut self, cmd_for: impl FnOnce(u64, Vec<u8>) -> AppCommand) {
        // One save at a time: worker threads do not serialize writes per target,
        // so two in-flight saves to the same path could land out of order and
        // leave stale bytes on disk while the UI reads clean.
        if self.pending_save.is_some() {
            self.set_toast("A save is already in progress".to_owned());
            return;
        }
        let Some(open) = &self.open_profile else {
            return;
        };
        let bytes = match open.session.serialize() {
            Ok(b) => b,
            Err(e) => {
                self.set_toast(e.to_string());
                return;
            }
        };
        let snapshot = open.session.current().clone();
        let req = self.alloc_req();
        self.pending_save = Some((req, snapshot));
        let cmd = cmd_for(req, bytes);
        self.worker.send(cmd);
    }

    pub(crate) fn save_in_place(&mut self) {
        let Some(open) = &self.open_profile else {
            return;
        };
        match open.source.clone() {
            ProfileSource::File(path) => {
                self.dispatch_save(|req, bytes| AppCommand::SaveFile { req, path, bytes });
            }
            ProfileSource::Device(name) => {
                self.dispatch_save(|req, bytes| AppCommand::SaveDevice { req, name, bytes });
            }
            // No in-place save target for a community source; button is disabled.
            ProfileSource::Community { .. } => {}
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn save_as(&mut self) {
        let Some(open) = &self.open_profile else {
            return;
        };
        // Seed the dialog with the source's name so accepting the default
        // does not scatter copies named "profile.csv".
        let file_name = match &open.source {
            ProfileSource::Device(name) => name.as_filename().to_owned(),
            ProfileSource::File(path) => path.file_name().map_or_else(
                || "profile.csv".to_owned(),
                |s| s.to_string_lossy().into_owned(),
            ),
            ProfileSource::Community { name, .. } => {
                format!(
                    "{}.csv",
                    yoke_volume::profile::sanitize_for_profile_name(name)
                )
            }
        };
        self.dispatch_save(|req, bytes| AppCommand::SaveAsDialog {
            req,
            bytes,
            file_name,
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn save_to_device(&mut self) {
        let Some(open) = &self.open_profile else {
            return;
        };
        let raw = match &open.source {
            ProfileSource::Device(name) => name.as_filename().to_owned(),
            ProfileSource::File(path) => path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            // Community titles are free-form ("Half-Life: Alyx"); coerce them
            // to a FAT-safe stem instead of bouncing the save with a toast.
            ProfileSource::Community { name, .. } => {
                yoke_volume::profile::sanitize_for_profile_name(name)
            }
        };
        match yoke_volume::ProfileName::new(&raw) {
            Ok(name) => {
                self.dispatch_save(|req, bytes| AppCommand::SaveDevice { req, name, bytes });
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }

    pub(crate) fn open_preview(&mut self) {
        let Some(open) = &self.open_profile else {
            return;
        };
        match open.session.serialize() {
            Ok(bytes) => {
                self.preview_csv = Some(String::from_utf8_lossy(&bytes).into_owned());
            }
            Err(e) => self.set_toast(e.to_string()),
        }
    }

    pub(crate) fn open_picker(&mut self, target: PickerTarget) {
        self.picker = Some(PickerState::new(self.selected_subprofile, target));
    }

    fn show_picker(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.picker.take() else {
            return;
        };
        let palette = self.palette;
        let outcome = {
            let session = &self
                .open_profile
                .as_ref()
                .expect("picker over open profile")
                .session;
            let sub = picker.sub;
            let input = match &picker.target {
                PickerTarget::AddBinding { input }
                | PickerTarget::EditOutput { input, .. }
                | PickerTarget::EditModifier { input, .. } => input.clone(),
            };
            let has = |m: &str| session.has_binding(sub, &input, m);
            crate::views::picker::show(ctx, &mut picker, &palette, &has)
        };
        // A commit that the engine refuses must not silently drop the picker:
        // the modal closed only on Open before, so a raced BindingExists or
        // ambiguous update would vanish behind a toast. Re-store the picker
        // with the error in capture_error on Err; close it on Ok (close-on-commit).
        let result = match outcome {
            crate::views::picker::PickerOutcome::Open => {
                self.picker = Some(picker);
                return;
            }
            crate::views::picker::PickerOutcome::Close => return,
            crate::views::picker::PickerOutcome::CommitOutput(output) => {
                self.commit_output(&picker, &output)
            }
            crate::views::picker::PickerOutcome::CommitModifier(modifier) => {
                self.commit_modifier(&picker, &modifier)
            }
        };
        if let Err(e) = result {
            let message = e.to_string();
            picker.capture_error = Some(message.clone());
            self.picker = Some(picker);
            self.set_toast(message);
        }
    }

    fn commit_output(
        &mut self,
        picker: &PickerState,
        output: &str,
    ) -> Result<(), yoke_edit::EditError> {
        let sub = picker.sub;
        match &picker.target {
            PickerTarget::AddBinding { input } => {
                let modifier = crate::edit::compose_modifier(&picker.keyword, &picker.args)
                    .expect("commit gated on valid modifier");
                let m = (modifier != "normal").then_some(modifier);
                self.open_profile
                    .as_mut()
                    .expect("open")
                    .session
                    .add_binding(sub, input, output, m.as_deref())
            }
            PickerTarget::EditOutput { input, modifier } => self
                .open_profile
                .as_mut()
                .expect("open")
                .session
                .update_binding(sub, input, output, modifier),
            PickerTarget::EditModifier { .. } => {
                unreachable!("modifier target commits a modifier")
            }
        }
    }

    fn commit_modifier(
        &mut self,
        picker: &PickerState,
        modifier: &str,
    ) -> Result<(), yoke_edit::EditError> {
        let sub = picker.sub;
        let PickerTarget::EditModifier { input, output, .. } = &picker.target else {
            unreachable!("output targets commit an output");
        };
        self.open_profile
            .as_mut()
            .expect("open")
            .session
            .update_binding(sub, input, output, modifier)
    }

    pub(crate) const fn subprofile_ui(&self) -> &SubProfileUi {
        &self.subprofile_ui
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) const fn has_toast(&self) -> bool {
        self.toast.is_some()
    }

    pub(crate) fn set_subprofile_ui(&mut self, ui: SubProfileUi) {
        self.subprofile_ui = ui;
    }

    pub(crate) fn edit_session_mut(&mut self) -> Option<&mut crate::edit::EditSession> {
        self.open_profile.as_mut().map(|o| &mut o.session)
    }

    /// Undo/redo route through these wrappers so the chip selection is
    /// re-clamped: undoing an add/clone shrinks the sub-profile list, and a
    /// selection past the end renders a blank editor with no chip highlighted.
    pub(crate) fn undo_edit(&mut self) {
        if let Some(s) = self.edit_session_mut() {
            s.undo();
            self.clamp_selected_subprofile();
        }
    }

    pub(crate) fn redo_edit(&mut self) {
        if let Some(s) = self.edit_session_mut() {
            s.redo();
            self.clamp_selected_subprofile();
        }
    }

    pub(crate) fn clamp_selected_subprofile(&mut self) {
        if let Some(open) = &self.open_profile {
            let len = open.session.current().sub_profiles.len();
            self.selected_subprofile = self.selected_subprofile.min(len.saturating_sub(1));
        }
    }

    /// Engine refusals surface as toasts; state was left untouched by `EditSession`.
    pub(crate) fn report_edit(&mut self, result: Result<(), yoke_edit::EditError>) {
        if let Err(e) = result {
            self.set_toast(e.to_string());
        }
    }
}

/// Shared builders for tests in sibling modules (e.g. `views::editor`) that
/// need a `YokeApp` with an open profile; they touch private fields so they
/// must live here.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_support {
    use super::{DataEvent, OpenInFlight, YokeApp};
    use crate::state::ProfileSource;

    #[must_use]
    pub fn test_app() -> YokeApp {
        let (_tx, events) = std::sync::mpsc::channel();
        YokeApp::new(crate::worker::WorkerHandle::for_test(), events, None, true)
    }

    #[must_use]
    pub fn a_profile() -> Box<yoke_config::ParseResult> {
        let csv = b"QuadStick Configuration,Version 1.4,,T\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";
        Box::new(yoke_config::parse(csv).expect("fixture parses"))
    }

    #[must_use]
    pub fn two_sub_profile() -> Box<yoke_config::ParseResult> {
        let csv = b"QuadStick Configuration,Version 1.4,,T\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n\
Profile Name,,Left Analog,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
kb_a,normal,left,\r\n\
\r\n";
        Box::new(yoke_config::parse(csv).expect("fixture parses"))
    }

    #[must_use]
    pub fn open_app_with(parsed: Box<yoke_config::ParseResult>) -> YokeApp {
        let mut app = test_app();
        let req = app.alloc_req();
        app.opening = Some(OpenInFlight {
            req,
            label: "test".into(),
        });
        app.apply_event(DataEvent::ProfileOpened {
            req,
            source: ProfileSource::File("/test.csv".into()),
            parsed,
        });
        app
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::test_support::{a_profile, open_app_with, test_app};
    use super::*;
    use crate::state::ProfileSource;

    #[test]
    fn stale_profile_opened_is_dropped() {
        let mut app = test_app();
        // The user opens A (req 0), then B (req 1) before A resolves: the latest
        // in-flight request is B.
        app.opening = Some(OpenInFlight {
            req: 0,
            label: "A".into(),
        });
        app.opening = Some(OpenInFlight {
            req: 1,
            label: "B".into(),
        });
        // B resolves first and is shown.
        app.apply_event(DataEvent::ProfileOpened {
            req: 1,
            source: ProfileSource::File("/b.csv".into()),
            parsed: a_profile(),
        });
        assert!(matches!(
            app.open_profile.as_ref().map(|o| &o.source),
            Some(ProfileSource::File(p)) if p.ends_with("b.csv")
        ));
        // A (stale, superseded by B) resolves later and must NOT clobber B.
        app.apply_event(DataEvent::ProfileOpened {
            req: 0,
            source: ProfileSource::File("/a.csv".into()),
            parsed: a_profile(),
        });
        assert!(
            matches!(
                app.open_profile.as_ref().map(|o| &o.source),
                Some(ProfileSource::File(p)) if p.ends_with("b.csv")
            ),
            "a stale open must not replace the newer profile"
        );
        assert!(app.opening.is_none());
    }

    #[test]
    fn open_community_failure_keeps_loaded_list() {
        let mut app = test_app();
        app.community = CommunityLoad::Loaded(std::sync::Arc::new(Vec::new()));
        app.opening = Some(OpenInFlight {
            req: 0,
            label: "Destiny 2".into(),
        });
        app.apply_event(DataEvent::Failed {
            req: Some(0),
            context: FailureContext::OpenCommunity,
            message: "503".into(),
        });
        // The browsed list survives; only a toast surfaces the per-entry failure.
        assert!(matches!(app.community, CommunityLoad::Loaded(_)));
        assert!(app.toast.is_some());
        assert!(app.opening.is_none());
    }

    #[test]
    fn list_device_failure_shows_no_toast() {
        let mut app = test_app();
        app.apply_event(DataEvent::Failed {
            req: None,
            context: FailureContext::ListDevice,
            message: "no QuadStick volume mounted".into(),
        });
        assert!(app.toast.is_none(), "cold-start device-less must not toast");
    }

    #[test]
    fn close_profile_with_dirty_session_prompts() {
        let mut app = open_app_with(a_profile());
        // lip is unbound in the a_profile fixture (only mouse_left is bound),
        // so add_binding succeeds and makes the session dirty.
        app.open_profile
            .as_mut()
            .unwrap()
            .session
            .add_binding(0, "lip", "kb_b", None)
            .unwrap();
        app.request_close_profile();
        assert!(app.open_profile.is_some(), "dirty close must not discard");
        assert!(app.confirm_discard);
    }

    #[test]
    fn close_profile_clean_session_closes() {
        let mut app = open_app_with(a_profile());
        app.request_close_profile();
        assert!(app.open_profile.is_none());
        assert!(!app.confirm_discard);
    }

    #[test]
    fn undo_redo_shortcuts_gated_to_editor_keyboard_ownership() {
        let mut app = open_app_with(a_profile());
        assert!(app.undo_redo_shortcuts_enabled(), "plain editor owns Cmd+Z");

        // egui::Modal does not block the keyboard, so a Cmd+Z behind the discard
        // prompt must not mutate the session and stale the prompt's premise.
        app.confirm_discard = true;
        assert!(!app.undo_redo_shortcuts_enabled());
        app.confirm_discard = false;

        // A focused rename/add TextEdit runs its own Cmd+Z undo; ceding the
        // shortcut avoids a silent double-act on the profile.
        app.set_subprofile_ui(SubProfileUi::Renaming {
            index: 0,
            value: String::new(),
        });
        assert!(!app.undo_redo_shortcuts_enabled());
        app.set_subprofile_ui(SubProfileUi::Closed);
        assert!(app.undo_redo_shortcuts_enabled());

        // An undo behind the preview modal would silently mutate the session
        // while the preview text stays stale.
        app.preview_csv = Some(String::new());
        assert!(!app.undo_redo_shortcuts_enabled());
        app.preview_csv = None;
        assert!(app.undo_redo_shortcuts_enabled());
    }

    #[test]
    fn saved_event_clears_dirty_and_toasts() {
        let mut app = open_app_with(a_profile());
        app.open_profile
            .as_mut()
            .unwrap()
            .session
            .add_binding(0, "lip", "kb_b", None)
            .unwrap();
        let snapshot = app.open_profile.as_ref().unwrap().session.current().clone();
        app.pending_save = Some((3, snapshot));
        app.apply_event(DataEvent::Saved {
            req: 3,
            label: "x.csv".into(),
        });
        assert!(!app.open_profile.as_ref().unwrap().session.is_dirty());
        assert!(app.toast.is_some());
    }

    #[test]
    fn stale_saved_event_is_dropped() {
        let mut app = open_app_with(a_profile());
        app.open_profile
            .as_mut()
            .unwrap()
            .session
            .add_binding(0, "lip", "kb_b", None)
            .unwrap();
        app.apply_event(DataEvent::Saved {
            req: 99,
            label: "x.csv".into(),
        });
        assert!(app.open_profile.as_ref().unwrap().session.is_dirty());
    }

    #[test]
    fn second_save_while_in_flight_is_refused() {
        let mut app = open_app_with(a_profile());
        let snapshot = app.open_profile.as_ref().unwrap().session.current().clone();
        app.pending_save = Some((1, snapshot));
        app.save_in_place();
        // The first save's slot must survive: a second concurrent write to the
        // same target could land out of order and leave stale bytes on disk.
        assert_eq!(app.pending_save.as_ref().map(|(r, _)| *r), Some(1));
        assert!(app.toast.is_some());
    }

    #[test]
    fn save_failure_clears_pending_and_stays_dirty() {
        let mut app = open_app_with(a_profile());
        app.open_profile
            .as_mut()
            .unwrap()
            .session
            .add_binding(0, "lip", "kb_b", None)
            .unwrap();
        let snapshot = app.open_profile.as_ref().unwrap().session.current().clone();
        app.pending_save = Some((5, snapshot));
        app.apply_event(DataEvent::Failed {
            req: Some(5),
            context: FailureContext::SaveFile,
            message: "disk full".into(),
        });
        assert!(app.pending_save.is_none());
        assert!(app.toast.is_some());
        // The write never landed; the session must still read as dirty.
        assert!(app.open_profile.as_ref().unwrap().session.is_dirty());
    }
}
