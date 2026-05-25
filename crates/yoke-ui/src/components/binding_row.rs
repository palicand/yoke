//! Single row in the bindings list.
//!
//! Renders the input trigger, modifier, and output for one [`Binding`]. Takes
//! the binding by value because the row is rebuilt whenever the parent's
//! signal-derived list changes; cloning a small struct is simpler than threading
//! lifetimes through Leptos' view tree.

use leptos::prelude::*;
use yoke_config::model::Binding;

#[component]
pub fn BindingRow(binding: Binding) -> impl IntoView {
    let Binding {
        output,
        modifier,
        input,
        comment: _,
    } = binding;
    let input_text = input.map_or_else(|| "(none)".to_owned(), |i| format!("{i:?}"));
    let modifier_text = format!("{modifier:?}");
    let output_text = format!("{output:?}");
    view! {
        <li class="qs-binding">
            <span class="qs-binding-when">"WHEN"</span>
            <span class="qs-binding-trigger">{input_text}</span>
            <span class="qs-binding-mod">{modifier_text}</span>
            <span class="qs-binding-arrow">"→"</span>
            <span class="qs-binding-out">{output_text}</span>
        </li>
    }
}
