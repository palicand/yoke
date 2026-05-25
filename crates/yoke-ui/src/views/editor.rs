//! Editor view shell shown when a profile is open.
//!
//! Hosts the [`EditorHeader`](crate::components::editor_header::EditorHeader),
//! the [`SubProfileStrip`](crate::components::sub_profile_strip::SubProfileStrip),
//! and the [`DeviceMap`](crate::components::device_map::DeviceMap), plus a
//! placeholder for the bindings panel that lands in the next task.

use leptos::prelude::*;

use crate::components::device_map::DeviceMap;
use crate::components::editor_header::EditorHeader;
use crate::components::sub_profile_strip::SubProfileStrip;

#[component]
pub fn EditorView() -> impl IntoView {
    let selected_subprofile = RwSignal::new(0usize);
    let selected_input = RwSignal::new(None::<String>);
    view! {
        <section class="qs-editor">
            <EditorHeader/>
            <SubProfileStrip selected=selected_subprofile/>
            <DeviceMap selected=selected_input/>
            <p class="qs-muted">"Bindings panel (next task)."</p>
        </section>
    }
}
