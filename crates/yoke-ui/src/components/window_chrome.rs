use leptos::prelude::*;
use yoke_ipc::VolumePresence;

use crate::state::use_state;

#[component]
pub fn WindowChrome() -> impl IntoView {
    let state = use_state();
    let pill = move || match state.volume.get() {
        VolumePresence::Present { label, .. } => ("connected", label),
        VolumePresence::DeviceVisibleNoVolume { mode_hint } => (
            "warning",
            // Hint values come from `ModeHint`: "MassStorageDisabled",
            // "Ps4OrHori", "Emulation". The first is the actionable one.
            match mode_hint.as_deref() {
                Some("MassStorageDisabled") => "Enable mass storage".to_string(),
                Some(other) => format!("Volume hidden ({other})"),
                None => "Volume hidden".to_string(),
            },
        ),
        VolumePresence::Absent => ("disconnected", "No device".to_string()),
    };
    view! {
        <header class="qs-chrome">
            <div class=move || format!("qs-pill qs-pill-{}", pill().0)>
                <span class="qs-pill-dot"></span>
                <span>{move || pill().1}</span>
            </div>
        </header>
    }
}
