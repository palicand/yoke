//! Editor view shell shown when a profile is open.
//!
//! Hosts the [`EditorHeader`](crate::components::editor_header::EditorHeader),
//! the [`SubProfileStrip`](crate::components::sub_profile_strip::SubProfileStrip),
//! the [`DeviceMap`](crate::components::device_map::DeviceMap), and the
//! [`BindingsPanel`](crate::components::bindings_panel::BindingsPanel).

use leptos::prelude::*;

use crate::components::bindings_panel::BindingsPanel;
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
            <div class="qs-editor-body">
                <DeviceMap selected=selected_input/>
                <BindingsPanel
                    selected_input=selected_input
                    selected_subprofile=selected_subprofile
                />
            </div>
        </section>
    }
}
