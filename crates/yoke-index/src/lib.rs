#![forbid(unsafe_code)]

pub mod cache;
pub mod client;
pub mod entry;
pub mod error;
pub mod source;
pub mod url_transform;

pub use client::IndexClient;
pub use entry::{IndexEntry, IndexListing};
pub use error::IndexError;
pub use source::ProfileSource;

pub const COMMUNITY_INDEX_URL: &str = "https://docs.google.com/spreadsheets/d/e/2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/pub?gid=1483029791&single=true&output=csv";

#[cfg(test)]
mod tests {
    use super::COMMUNITY_INDEX_URL;

    #[test]
    fn community_index_url_has_no_embedded_whitespace() {
        assert!(!COMMUNITY_INDEX_URL.chars().any(char::is_whitespace));
    }
}
