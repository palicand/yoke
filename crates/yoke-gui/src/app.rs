use crate::data::{AppCommand, DataEvent, FailureContext};
use crate::state::{CommunityLoad, OpenProfile};
use crate::theme::Palette;

#[cfg(not(target_arch = "wasm32"))]
use yoke_volume::state::MountState;
#[cfg(target_arch = "wasm32")]
use crate::data::mock::MockMountState as MountState;

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
            requested_initial: false,
        }
    }

    #[cfg(target_arch = "wasm32")]
    #[must_use]
    // WorkerHandle is not const-constructible.
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(worker: crate::worker::WorkerHandle) -> Self {
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
            DataEvent::VolumeChanged(state) => self.volume = Some(state),
            DataEvent::ProfileOpened { source, profile } => {
                self.selected_station = None;
                self.selected_subprofile = 0;
                self.open_profile = Some(OpenProfile { source, profile: *profile });
            }
            DataEvent::Failed { context, message } => self.handle_failure(context, message),
        }
    }

    fn handle_failure(&mut self, context: FailureContext, message: String) {
        if context == FailureContext::OpenFile && message.is_empty() {
            return; // dialog cancelled
        }
        if context == FailureContext::ListCommunity {
            self.community = CommunityLoad::Failed(message);
            return;
        }
        if context == FailureContext::OpenCommunity {
            self.community = CommunityLoad::Failed(message.clone());
        }
        self.set_toast(message);
    }

    fn set_toast(&mut self, message: String) {
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
            self.worker.send(AppCommand::ListCommunity);
        }

        let ctx = ui.ctx().clone();

        egui::Panel::top("yoke_top").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Yoke");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.status_pill(ui);
                });
            });
        });

        egui::Panel::left("yoke_rail").resizable(false).default_size(160.0).show_inside(ui, |ui| {
            ui.add_space(8.0);
            let on_library = self.open_profile.is_none();
            if ui.selectable_label(on_library, "Profiles").clicked() {
                self.open_profile = None;
                self.selected_station = None;
            }
            ui.separator();
            ui.label(egui::RichText::new("DEVICE").small().color(self.palette.ink_3));
            self.rail_device_status(ui);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.open_profile.is_some() {
                crate::views::editor::show(self, ui);
            } else {
                crate::views::library::show(self, ui);
            }
        });

        self.show_toast(&ctx, ui);

        // Escape steps back: clear station, then close profile.
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.selected_station.is_some() {
                self.selected_station = None;
            } else if self.open_profile.is_some() {
                self.open_profile = None;
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
            Some(_) | None => ("Disconnected", self.palette.ink_3),
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
                .on_hover_text("Volume backend failed to initialize; file-open and community still work.");
        }
    }

    #[allow(clippy::float_cmp)] // f64::MAX is a sentinel meaning "not yet set"; exact equality is intentional
    fn show_toast(&mut self, ctx: &egui::Context, _ui: &egui::Ui) {
        let now = ctx.input(|i| i.time);
        let Some((msg, expiry)) = self.toast.as_mut() else { return };
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

    pub(crate) const fn palette(&self) -> &Palette { &self.palette }
    pub(crate) fn device_profiles(&self) -> &[crate::data::ProfileEntryView] { &self.device_profiles }
    pub(crate) const fn community(&self) -> &CommunityLoad { &self.community }
    pub(crate) const fn open_profile(&self) -> Option<&OpenProfile> { self.open_profile.as_ref() }
    pub(crate) const fn selected_station(&self) -> Option<&'static str> { self.selected_station }
    pub(crate) const fn set_selected_station(&mut self, s: Option<&'static str>) { self.selected_station = s; }
    pub(crate) const fn selected_subprofile(&self) -> usize { self.selected_subprofile }
    pub(crate) const fn set_selected_subprofile(&mut self, i: usize) { self.selected_subprofile = i; }
    pub(crate) fn close_profile(&mut self) { self.open_profile = None; self.selected_station = None; }
    pub(crate) fn send(&self, cmd: AppCommand) { self.worker.send(cmd); }
}
