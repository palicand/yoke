//! Indeterminate loading spinner.
//!
//! CSS-only ring (no JS, no deps). The animation is dropped under
//! `prefers-reduced-motion`; see `.qs-spinner` in `styles/components.css`.

use leptos::prelude::*;

#[component]
pub fn Spinner() -> impl IntoView {
    view! { <span class="qs-spinner" role="status" aria-label="Loading"></span> }
}
