use crate::data::{AppCommand, DataEvent, FailureContext};
use crate::state::{CommunityLoad, OpenProfile};
use crate::theme::Palette;

#[cfg(target_arch = "wasm32")]
use crate::data::mock::{MockCommunityEntry as IndexEntry, MockMountState as MountState};
#[cfg(not(target_arch = "wasm32"))]
use yoke_volume::state::{ModeHint, MountState};
#[cfg(not(target_arch = "wasm32"))]
use {yoke_index::IndexEntry, yoke_volume::ProfileName};

#[cfg(target_arch = "wasm32")]
type ProfileName = String;

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

pub struct YokeApp {
    palette: Palette,
    worker: crate::worker::WorkerHandle,
    #[cfg(not(target_arch = "wasm32"))]
    events: std::sync::mpsc::Receiver<DataEvent>,

    volume: Option<MountState>,
    backend_error: Option<String>,
    device_profiles: Vec<crate::data::ProfileEntryView>,
    community: CommunityLoad,
    open_profile: Option<OpenProfile>,
    selected_station: Option<&'static str>,
    selected_subprofile: usize,
    toast: Option<(String, f64)>,
    // Task 5 renders this; the field exists now so dispatch can store state.
    #[allow(dead_code)]
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
            community: CommunityLoad::Loading,
            open_profile: None,
            selected_station: None,
            selected_subprofile: 0,
            toast: None,
            picker: None,
            opening: None,
            next_req: 0,
            community_available,
            requested_initial: false,
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
            community: CommunityLoad::Loading,
            open_profile: None,
            selected_station: None,
            selected_subprofile: 0,
            toast: None,
            picker: None,
            opening: None,
            next_req: 0,
            community_available,
            requested_initial: false,
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

    fn apply_event(&mut self, ev: DataEvent) {
        match ev {
            DataEvent::ProfilesListed(list) => self.device_profiles = list,
            DataEvent::CommunityListed(list) => self.community = CommunityLoad::Loaded(list),
            DataEvent::VolumeChanged(state) => {
                // The volume watcher fires this on mount/unmount, so the device
                // list tracks the device live.
                match volume_action(self.volume.as_ref(), &state) {
                    VolumeAction::Relist => self.worker.send(AppCommand::ListDeviceProfiles),
                    VolumeAction::Clear => self.device_profiles.clear(),
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
                self.open_profile = Some(OpenProfile {
                    source,
                    session: crate::edit::EditSession::new(*parsed),
                });
            }
            DataEvent::FileDialogCancelled { req } => {
                if self.opening.as_ref().map(|o| o.req) == Some(req) {
                    self.opening = None;
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
            // The empty Library + "Disconnected" pill already convey "no device";
            // a red toast on every device-less cold start is noise.
            FailureContext::ListDevice => {}
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
            self.worker.send(AppCommand::ListDeviceProfiles);
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
            .default_size(180.0)
            .frame(rail_frame)
            .show_inside(ui, |ui| {
                let on_library = self.open_profile.is_none();
                if ui.selectable_label(on_library, "Profiles").clicked() {
                    self.close_profile();
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

        // Escape steps back: station selection, then the open profile, then a
        // pending open (dismiss the loading overlay if the user backs out before
        // the worker returns).
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.selected_station.is_some() {
                self.selected_station = None;
            } else if self.open_profile.is_some() {
                self.close_profile();
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
        ui.colored_label(color, text);
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
    pub(crate) const fn community(&self) -> &CommunityLoad {
        &self.community
    }
    pub(crate) const fn open_profile(&self) -> Option<&OpenProfile> {
        self.open_profile.as_ref()
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
        // Backing out also cancels a pending open so its loading overlay stops
        // painting over the Library; the in-flight result is dropped on arrival.
        self.opening = None;
    }
    pub(crate) fn send(&self, cmd: AppCommand) {
        self.worker.send(cmd);
    }

    pub(crate) fn open_picker(&mut self, target: PickerTarget) {
        self.picker = Some(PickerState::new(self.selected_subprofile, target));
    }

    pub(crate) fn edit_session_mut(&mut self) -> Option<&mut crate::edit::EditSession> {
        self.open_profile.as_mut().map(|o| &mut o.session)
    }

    /// Engine refusals surface as toasts; state was left untouched by `EditSession`.
    pub(crate) fn report_edit(&mut self, result: Result<(), yoke_edit::EditError>) {
        if let Err(e) = result {
            self.set_toast(e.to_string());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::state::ProfileSource;

    fn test_app() -> YokeApp {
        let (_tx, events) = std::sync::mpsc::channel();
        YokeApp::new(crate::worker::WorkerHandle::for_test(), events, None, true)
    }

    fn a_profile() -> Box<yoke_config::ParseResult> {
        let csv = b"QuadStick Configuration,Version 1.4,,T\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";
        Box::new(yoke_config::parse(csv).expect("fixture parses"))
    }

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
        app.community = CommunityLoad::Loaded(Vec::new());
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
}
