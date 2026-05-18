use std::collections::BTreeMap;
use url::Url;

use crate::IndexError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub name: String,
    pub csv_url: Url,
    pub fields: BTreeMap<String, String>,
}

pub fn parse_index(bytes: &[u8]) -> Result<(Vec<IndexEntry>, usize), IndexError> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(bytes);
    let headers = rdr
        .headers()
        .map_err(|e| IndexError::IndexFormat(e.to_string()))?
        .clone();
    let name_col = headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case("name"))
        .ok_or_else(|| IndexError::IndexFormat("missing 'name' column".into()))?;
    let url_col = headers
        .iter()
        .position(|h| {
            let h = h.to_ascii_lowercase();
            h.contains("csv") || h.contains("link") || h.contains("url")
        })
        .ok_or_else(|| IndexError::IndexFormat("missing csv/link/url column".into()))?;

    let mut entries = Vec::new();
    let mut skipped = 0_usize;
    for rec in rdr.records() {
        let rec = rec.map_err(|e| IndexError::IndexFormat(e.to_string()))?;
        let name = rec.get(name_col).unwrap_or("").trim().to_string();
        let raw_url = rec.get(url_col).unwrap_or("").trim();
        if name.is_empty() || raw_url.is_empty() {
            skipped += 1;
            tracing::warn!("index row skipped: empty name or url");
            continue;
        }
        let Ok(csv_url) = Url::parse(raw_url) else {
            skipped += 1;
            tracing::warn!(raw_url, "skipping row with unparseable url");
            continue;
        };
        let mut fields = BTreeMap::new();
        for (i, h) in headers.iter().enumerate() {
            if i == name_col || i == url_col {
                continue;
            }
            if let Some(v) = rec.get(i) {
                fields.insert(h.to_string(), v.to_string());
            }
        }
        entries.push(IndexEntry {
            name,
            csv_url,
            fields,
        });
    }
    Ok((entries, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_index() {
        let csv = b"Name,CSV URL\nDestiny 2,https://example.org/d2.csv\n";
        let (entries, skipped) = parse_index(csv).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Destiny 2");
        assert_eq!(skipped, 0);
    }

    #[test]
    fn preserves_extra_columns_in_fields() {
        let csv = b"Name,CSV URL,Author,Updated\nD2,https://x.example/d2.csv,Alice,2026-01-01\n";
        let (entries, _) = parse_index(csv).unwrap();
        assert_eq!(entries[0].fields.get("Author").unwrap(), "Alice");
    }

    #[test]
    fn skips_unparseable_rows_and_counts_them() {
        let csv = b"Name,CSV URL\n,\nOk,https://example.org/ok.csv\nBad,not-a-url\n";
        let (entries, skipped) = parse_index(csv).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(skipped, 2);
    }

    #[test]
    fn rejects_missing_name_column() {
        let csv = b"Foo,CSV URL\nx,https://x.example\n";
        assert!(matches!(parse_index(csv), Err(IndexError::IndexFormat(_))));
    }
}
