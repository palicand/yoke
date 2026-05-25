//! Tauri-backed [`Backend`] used when the UI runs inside the Tauri shell.
//!
//! Bridges to the IPC commands and the `yoke://volume-state` event emitted by
//! the `yoke-tauri` host. The trait requires futures and streams to be `Send`,
//! but `tauri-sys` returns `!Send` futures and streams (they capture JS
//! closures). WASM is single-threaded, so [`SendWrapper`] is sound here: it
//! panics only if the wrapped value is accessed off the originating thread,
//! which cannot happen.

use std::path::PathBuf;

use async_stream::stream;
use futures::StreamExt;
use send_wrapper::SendWrapper;
use serde::Serialize;
use tauri_sys::core::{invoke, invoke_result};
use tauri_sys::event::listen;
use yoke_ipc::{BackendError, CommunityEntry, DeviceProfileEntry, Profile, VolumePresence};

use super::{Backend, BackendFuture, VolumeStream};

const VOLUME_EVENT: &str = "yoke://volume-state";

pub struct TauriBackend;

impl TauriBackend {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for TauriBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct EmptyArgs {}

#[derive(Serialize)]
struct NameArg {
    name: String,
}

#[derive(Serialize)]
struct UrlArg {
    url: String,
}

#[derive(Serialize)]
struct PathArg {
    path: PathBuf,
}

impl Backend for TauriBackend {
    fn volume_state(&self) -> BackendFuture<'_, VolumePresence> {
        Box::pin(SendWrapper::new(async {
            Ok(invoke::<VolumePresence>("volume_state", &EmptyArgs {}).await)
        }))
    }

    fn watch_volume_state(&self) -> VolumeStream {
        Box::pin(SendWrapper::new(stream! {
            match listen::<VolumePresence>(VOLUME_EVENT).await {
                Ok(mut events) => {
                    while let Some(event) = events.next().await {
                        yield event.payload;
                    }
                }
                Err(err) => {
                    tracing::warn!(error = ?err, "failed to subscribe to volume event");
                }
            }
        }))
    }

    fn list_device_profiles(&self) -> BackendFuture<'_, Vec<DeviceProfileEntry>> {
        Box::pin(SendWrapper::new(async {
            invoke_result::<Vec<DeviceProfileEntry>, BackendError>(
                "list_device_profiles",
                &EmptyArgs {},
            )
            .await
        }))
    }

    fn read_device_profile(&self, name: String) -> BackendFuture<'_, Profile> {
        Box::pin(SendWrapper::new(async move {
            invoke_result::<Profile, BackendError>("read_device_profile", &NameArg { name }).await
        }))
    }

    fn pick_file_dialog(&self) -> BackendFuture<'_, Option<PathBuf>> {
        Box::pin(SendWrapper::new(async {
            Ok(invoke::<Option<PathBuf>>("pick_file_dialog", &EmptyArgs {}).await)
        }))
    }

    fn read_file_profile(&self, path: PathBuf) -> BackendFuture<'_, Profile> {
        Box::pin(SendWrapper::new(async move {
            invoke_result::<Profile, BackendError>("read_file_profile", &PathArg { path }).await
        }))
    }

    fn list_community_profiles(&self) -> BackendFuture<'_, Vec<CommunityEntry>> {
        Box::pin(SendWrapper::new(async {
            invoke_result::<Vec<CommunityEntry>, BackendError>(
                "list_community_profiles",
                &EmptyArgs {},
            )
            .await
        }))
    }

    fn fetch_community_profile(&self, url: String) -> BackendFuture<'_, Profile> {
        Box::pin(SendWrapper::new(async move {
            invoke_result::<Profile, BackendError>("fetch_community_profile", &UrlArg { url }).await
        }))
    }
}
