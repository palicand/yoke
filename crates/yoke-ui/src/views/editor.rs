//! Editor view shell shown when a profile is open.
//!
//! Hosts the [`EditorHeader`](crate::components::editor_header::EditorHeader)
//! and the [`SubProfileStrip`](crate::components::sub_profile_strip::SubProfileStrip),
//! plus placeholders for the device map and bindings panel that land in
//! subsequent tasks.

use leptos::prelude::*;

use crate::components::editor_header::EditorHeader;
use crate::components::sub_profile_strip::SubProfileStrip;

#[component]
pub fn EditorView() -> impl IntoView {
    let selected = RwSignal::new(0usize);
    view! {
        <section class="qs-editor">
            <EditorHeader/>
            <SubProfileStrip selected=selected/>
            <p class="qs-muted">"Device map and bindings panel (next tasks)."</p>
        </section>
    }
}
