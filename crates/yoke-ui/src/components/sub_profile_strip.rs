//! Sub-profile chip strip beneath the editor header.
//!
//! Renders one chip per sub-profile in the currently open profile and lets the
//! user pick which sub-profile is active. The selected index is owned by the
//! parent ([`EditorView`](crate::views::editor::EditorView)) so sibling panels
//! can read it without re-deriving from the profile contents.

use leptos::prelude::*;

use crate::state::use_state;

#[component]
pub fn SubProfileStrip(selected: RwSignal<usize>) -> impl IntoView {
    let state = use_state();
    let items = move || {
        state
            .open_profile
            .get()
            .map(|p| {
                p.profile
                    .sub_profiles
                    .iter()
                    .enumerate()
                    .map(|(i, sp)| {
                        (
                            i,
                            sp.header.profile_name.clone(),
                            format!("{:?}", sp.header.mode),
                            sp.bindings().count(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    view! {
        <div class="qs-strip">
            <For
                each=items
                key=|(i, name, _, _)| (*i, name.clone())
                children=move |(i, name, mode, count)| {
                    let is_active = move || selected.get() == i;
                    view! {
                        <button
                            class=move || if is_active() { "qs-chip qs-chip-active" } else { "qs-chip" }
                            on:click=move |_| selected.set(i)
                        >
                            <span class="qs-chip-name">{name}</span>
                            <span class="qs-chip-mode">{mode}</span>
                            <span class="qs-chip-count">{count}</span>
                        </button>
                    }
                }
            />
        </div>
    }
}
