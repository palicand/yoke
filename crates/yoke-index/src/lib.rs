#![forbid(unsafe_code)]

pub mod cache;
pub mod client;
pub mod entry;
pub mod error;
pub mod source;
pub mod url_transform;

pub use client::{IndexClient, fetch_profile_bytes};
pub use entry::{IndexEntry, IndexListing};
pub use error::IndexError;
pub use source::ProfileSource;

pub const COMMUNITY_INDEX_URL: &str = "https://docs.google.com/spreadsheets/d/e/2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/pub?gid=1483029791&single=true&output=csv";

// Browser-facing companion to COMMUNITY_INDEX_URL. Hand-paired rather than
// derived because the forward transform in url_transform.rs goes html → csv,
// and introducing a reverse path for one constant would muddy the semantics.
pub const COMMUNITY_INDEX_HTML_URL: &str = "https://docs.google.com/spreadsheets/d/e/2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/pubhtml?gid=1483029791&single=true";

#[cfg(test)]
mod tests {
    use super::{COMMUNITY_INDEX_HTML_URL, COMMUNITY_INDEX_URL};
    use url::Url;

    #[test]
    fn community_index_urls_have_no_embedded_whitespace() {
        assert!(!COMMUNITY_INDEX_URL.chars().any(char::is_whitespace));
        assert!(!COMMUNITY_INDEX_HTML_URL.chars().any(char::is_whitespace));
    }

    #[test]
    fn community_index_url_pair_targets_same_sheet_and_tab() {
        // Hand-paired CSV and pubhtml URLs must reference the same published
        // sheet (the `/e/<token>/` segment) and the same tab (`gid=<id>`).
        // Rotating one without the other silently sends `index browse` and
        // `index list` to different sheets.
        let csv = Url::parse(COMMUNITY_INDEX_URL).unwrap();
        let html = Url::parse(COMMUNITY_INDEX_HTML_URL).unwrap();
        assert_eq!(token_after_e(&csv), token_after_e(&html));
        assert_eq!(gid(&csv), gid(&html));
    }

    fn token_after_e(url: &Url) -> String {
        let segments: Vec<&str> = url.path_segments().map(Iterator::collect).unwrap_or_default();
        let i = segments
            .iter()
            .position(|s| *s == "e")
            .expect("path contains /e/ segment");
        segments
            .get(i + 1)
            .copied()
            .expect("path has token after /e/")
            .to_string()
    }

    fn gid(url: &Url) -> String {
        url.query_pairs()
            .find(|(k, _)| k == "gid")
            .map(|(_, v)| v.into_owned())
            .expect("query has gid")
    }
}
