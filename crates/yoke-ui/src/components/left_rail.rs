use leptos::prelude::*;
use yoke_ipc::VolumePresence;

use crate::state::use_state;

#[component]
pub fn LeftRail() -> impl IntoView {
    let state = use_state();
    let device_line = move || match state.volume.get() {
        VolumePresence::Present { label, .. } => format!("Connected · {label}"),
        VolumePresence::Absent => "No device".into(),
        VolumePresence::DeviceVisibleNoVolume { mode_hint } => mode_hint.map_or_else(
            || "Visible (no volume)".into(),
            |h| format!("Visible · {h}"),
        ),
    };
    view! {
        <nav class="qs-rail">
            <ul class="qs-rail-section">
                <li class="qs-rail-item qs-rail-item-active">"Profiles"</li>
            </ul>
            <div class="qs-rail-section">
                <div class="qs-rail-eyebrow">"DEVICE"</div>
                <div class="qs-rail-device">{device_line}</div>
            </div>
        </nav>
    }
}
