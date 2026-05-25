//! Header bar for the editor view.
//!
//! Renders a back-to-library button, a breadcrumb describing the
//! [`ProfileSource`] of the currently open profile, and metadata counts
//! (total bindings and sub-profile count).

use leptos::prelude::*;

use crate::state::{ProfileSource, use_state};

#[component]
pub fn EditorHeader() -> impl IntoView {
    let state = use_state();
    let open_profile = state.open_profile;
    let breadcrumb = move || match open_profile.get() {
        Some(p) => match p.source {
            ProfileSource::Device(name) => format!("DEVICE · {name}"),
            ProfileSource::File(path) => format!("FILE · {}", path.display()),
            ProfileSource::Community { name, .. } => format!("COMMUNITY · {name}"),
        },
        None => String::new(),
    };
    let metadata = move || match open_profile.get() {
        Some(p) => {
            let bindings: usize = p
                .profile
                .sub_profiles
                .iter()
                .map(|sp| sp.bindings().count())
                .sum();
            format!(
                "{} bindings · {} sub-profiles",
                bindings,
                p.profile.sub_profiles.len()
            )
        }
        None => String::new(),
    };
    let on_back = move |_| open_profile.set(None);
    view! {
        <header class="qs-editor-header">
            <button class="qs-back" on:click=on_back>"← Library"</button>
            <div class="qs-crumb">{breadcrumb}</div>
            <div class="qs-meta">{metadata}</div>
        </header>
    }
}
