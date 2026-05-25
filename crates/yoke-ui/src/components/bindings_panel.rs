//! Bindings list panel beside the device map.
//!
//! Reads the open profile's currently-selected sub-profile and renders each
//! [`Binding`] as a [`BindingRow`]. When a station is selected in the
//! [`DeviceMap`](super::device_map::DeviceMap), the list is filtered via
//! [`input_belongs_to`]; with no selection the panel shows every binding.

use leptos::prelude::*;
use yoke_config::model::Binding;

use super::binding_row::BindingRow;
use super::stations::input_belongs_to;
use crate::state::use_state;

#[component]
pub fn BindingsPanel(
    selected_input: RwSignal<Option<String>>,
    selected_subprofile: RwSignal<usize>,
) -> impl IntoView {
    let state = use_state();
    let bindings = move || -> Vec<Binding> {
        let Some(open) = state.open_profile.get() else {
            return Vec::new();
        };
        let idx = selected_subprofile.get();
        let Some(sub) = open.profile.sub_profiles.get(idx) else {
            return Vec::new();
        };
        let filter = selected_input.get();
        sub.bindings()
            .filter(|b| match (&filter, &b.input) {
                (Some(station_id), Some(input)) => input_belongs_to(input, station_id),
                (Some(_), None) => false,
                (None, _) => true,
            })
            .cloned()
            .collect()
    };
    let header = move || {
        selected_input
            .get()
            .map_or_else(|| "ALL".into(), |id| id.to_uppercase())
    };
    view! {
        <aside class="qs-bindings">
            <div class="qs-bindings-eyebrow">{header}</div>
            <ul class="qs-binding-list">
                <For
                    each=bindings
                    key=|b: &Binding| format!("{b:?}")
                    children=move |b: Binding| view! { <BindingRow binding=b/> }
                />
            </ul>
        </aside>
    }
}
