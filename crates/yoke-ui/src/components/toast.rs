//! Auto-dismissing toast banner driven by `AppState::toast`.
//!
//! Renders the current message when `state.toast` is `Some`, then schedules a
//! 5-second timer to clear it. A new toast arriving while a timer is in flight
//! will be cleared early by the older timer; that is a known v1 limitation.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::state::use_state;

#[component]
pub fn Toast() -> impl IntoView {
    let state = use_state();
    let toast = state.toast;
    Effect::new(move |_| {
        if toast.get().is_some() {
            spawn_local(async move {
                gloo_timers::future::sleep(Duration::from_secs(5)).await;
                toast.set(None);
            });
        }
    });
    view! {
        {move || toast.get().map(|msg| view! { <aside class="qs-toast">{msg}</aside> })}
    }
}
