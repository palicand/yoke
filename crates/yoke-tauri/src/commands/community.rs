use std::sync::Arc;
use std::time::Duration;

use yoke_config::parse;
use yoke_index::{IndexClient, IndexEntry, ProfileSource, fetch_profile_bytes};
use yoke_ipc::{BackendError, CommunityEntry, Profile};

pub struct CommunityState {
    client: Arc<IndexClient>,
}

impl CommunityState {
    pub fn new() -> Result<Self, anyhow::Error> {
        let client = IndexClient::new()?.with_cache(
            std::env::temp_dir().join("yoke-community-cache"),
            Duration::from_mins(5),
        );
        Ok(Self {
            client: Arc::new(client),
        })
    }
}

#[tauri::command]
pub async fn list_community_profiles(
    state: tauri::State<'_, CommunityState>,
) -> Result<Vec<CommunityEntry>, BackendError> {
    let client = Arc::clone(&state.client);
    let listing = client
        .list(false)
        .await
        .map_err(|e| BackendError::Network(e.to_string()))?;
    Ok(listing.entries.into_iter().map(to_dto).collect())
}

#[tauri::command]
pub async fn fetch_community_profile(url: String) -> Result<Profile, BackendError> {
    let parsed_url = url::Url::parse(&url).map_err(|e| BackendError::Parse(e.to_string()))?;
    let bytes = fetch_profile_bytes(ProfileSource::Url(parsed_url))
        .await
        .map_err(|e| BackendError::Network(e.to_string()))?;
    let parsed = parse(&bytes).map_err(|e| BackendError::Parse(e.to_string()))?;
    Ok(parsed.model)
}

fn to_dto(entry: IndexEntry) -> CommunityEntry {
    CommunityEntry {
        name: entry.name,
        url: entry.csv_url.to_string(),
        fields: entry.fields,
    }
}

#[cfg(test)]
mod tests {
    use super::{IndexEntry, to_dto};
    use std::collections::BTreeMap;
    use url::Url;

    #[test]
    fn to_dto_is_lossless_for_required_fields() {
        let mut fields = BTreeMap::new();
        fields.insert("variant".to_string(), "FPS".to_string());
        let entry = IndexEntry {
            name: "Alice".into(),
            csv_url: Url::parse("https://example.invalid/a.csv").unwrap(),
            fields: fields.clone(),
        };
        let dto = to_dto(entry);
        assert_eq!(dto.name, "Alice");
        assert_eq!(dto.url, "https://example.invalid/a.csv");
        assert_eq!(dto.fields, fields);
    }
}
