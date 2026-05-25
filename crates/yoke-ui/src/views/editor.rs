//! Editor view shell shown when a profile is open.
//!
//! Hosts the [`EditorHeader`](crate::components::editor_header::EditorHeader)
//! plus placeholders for the sub-profile strip, device map, and bindings
//! panel that land in subsequent tasks.

use leptos::prelude::*;

use crate::components::editor_header::EditorHeader;

#[component]
pub fn EditorView() -> impl IntoView {
    view! {
        <section class="qs-editor">
            <EditorHeader/>
            <p class="qs-muted">"Sub-profile strip, device map, and bindings panel (Tasks 20–22)."</p>
        </section>
    }
}
