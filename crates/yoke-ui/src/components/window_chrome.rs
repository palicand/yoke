use leptos::prelude::*;
use yoke_ipc::VolumePresence;

use crate::state::use_state;

#[component]
pub fn WindowChrome() -> impl IntoView {
    let state = use_state();
    let pill = move || match state.volume.get() {
        VolumePresence::Present { .. } => ("connected", "Connected"),
        VolumePresence::Absent | VolumePresence::DeviceVisibleNoVolume { .. } => {
            ("disconnected", "Disconnected")
        }
    };
    view! {
        <header class="qs-chrome">
            <div class="qs-lights">
                <span class="qs-light qs-light-r"></span>
                <span class="qs-light qs-light-y"></span>
                <span class="qs-light qs-light-g"></span>
            </div>
            <div class="qs-title">"QuadStick · Configuration"</div>
            <div class=move || format!("qs-pill qs-pill-{}", pill().0)>
                <span class="qs-pill-dot"></span>
                <span>{move || pill().1}</span>
            </div>
        </header>
    }
}
