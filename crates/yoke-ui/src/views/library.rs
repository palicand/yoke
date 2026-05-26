//! Landing view listing the three profile sources: device, file, community.
//!
//! The user picks one entry and the view writes [`OpenProfile`] into
//! `state.open_profile`; the App swaps to the editor on the next render.
//! Failures surface via `state.toast` rather than blocking the UI.

use leptos::prelude::*;
use leptos::task::spawn_local;
use yoke_ipc::{CommunityEntry, DeviceProfileEntry};

use crate::components::spinner::Spinner;
use crate::state::{AppState, CommunityLoad, OpenProfile, ProfileSource, use_state};

#[component]
pub fn LibraryView() -> impl IntoView {
    let state = use_state();
    view! {
        <section class="qs-library">
            <DeviceProfileList state=state.clone()/>
            <OpenFileAction state=state.clone()/>
            <CommunityProfileList state=state/>
        </section>
    }
}

#[component]
fn DeviceProfileList(state: AppState) -> impl IntoView {
    let device_profiles = state.device_profiles;
    view! {
        <div class="qs-section">
            <h3>"From device"</h3>
            <Show
                when=move || !device_profiles.get().is_empty()
                fallback=|| view! { <p class="qs-muted">"No device profiles."</p> }
            >
                {
                    let state = state.clone();
                    move || {
                        let state = state.clone();
                        view! {
                            <ul class="qs-list">
                                <For
                                    each=move || device_profiles.get()
                                    key=|e: &DeviceProfileEntry| e.name.clone()
                                    let:entry
                                >
                                    {
                                        let state = state.clone();
                                        let name = entry.name.clone();
                                        view! {
                                            <li
                                                class="qs-list-item"
                                                on:click=move |_| open_device(state.clone(), name.clone())
                                            >
                                                {entry.name}
                                            </li>
                                        }
                                    }
                                </For>
                            </ul>
                        }
                    }
                }
            </Show>
        </div>
    }
}

#[component]
fn OpenFileAction(state: AppState) -> impl IntoView {
    view! {
        <div class="qs-section">
            <button class="qs-button" on:click=move |_| open_file(state.clone())>
                "Open profile file..."
            </button>
        </div>
    }
}

#[component]
fn CommunityProfileList(state: AppState) -> impl IntoView {
    let community = state.community;
    view! {
        <div class="qs-section">
            <h3>"Community"</h3>
            {move || match community.get() {
                CommunityLoad::Loading => view! {
                    <div class="qs-loading">
                        <Spinner/>
                        <span class="qs-muted">"Loading community profiles…"</span>
                    </div>
                }
                .into_any(),
                CommunityLoad::Failed(_) => view! {
                    <p class="qs-muted">"Couldn't load community profiles."</p>
                }
                .into_any(),
                CommunityLoad::Loaded(entries) if entries.is_empty() => view! {
                    <p class="qs-muted">"No community profiles."</p>
                }
                .into_any(),
                CommunityLoad::Loaded(entries) => {
                    let state = state.clone();
                    view! {
                        <ul class="qs-list">
                            <For
                                each=move || entries.clone()
                                key=|e: &CommunityEntry| e.url.clone()
                                let:entry
                            >
                                {
                                    let state = state.clone();
                                    let name = entry.name.clone();
                                    let url = entry.url.clone();
                                    view! {
                                        <li
                                            class="qs-list-item"
                                            on:click=move |_| open_community(state.clone(), name.clone(), url.clone())
                                        >
                                            {entry.name}
                                        </li>
                                    }
                                }
                            </For>
                        </ul>
                    }
                    .into_any()
                }
            }}
        </div>
    }
}

fn open_device(state: AppState, name: String) {
    spawn_local(async move {
        match state.backend.read_device_profile(name.clone()).await {
            Ok(profile) => state.open_profile.set(Some(OpenProfile {
                source: ProfileSource::Device(name),
                profile,
            })),
            Err(e) => state.toast.set(Some(format!("Read failed: {e}"))),
        }
    });
}

fn open_file(state: AppState) {
    spawn_local(async move {
        let path = match state.backend.pick_file_dialog().await {
            Ok(Some(p)) => p,
            Ok(None) => return,
            Err(e) => {
                state.toast.set(Some(format!("Dialog failed: {e}")));
                return;
            }
        };
        match state.backend.read_file_profile(path.clone()).await {
            Ok(profile) => state.open_profile.set(Some(OpenProfile {
                source: ProfileSource::File(path),
                profile,
            })),
            Err(e) => state.toast.set(Some(format!("Parse failed: {e}"))),
        }
    });
}

fn open_community(state: AppState, name: String, url: String) {
    spawn_local(async move {
        match state.backend.fetch_community_profile(url.clone()).await {
            Ok(profile) => state.open_profile.set(Some(OpenProfile {
                source: ProfileSource::Community { name, url },
                profile,
            })),
            Err(e) => state
                .toast
                .set(Some(format!("Community fetch failed: {e}"))),
        }
    });
}
