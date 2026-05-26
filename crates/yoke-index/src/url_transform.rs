use url::Url;

use crate::IndexError;

pub fn to_csv_export(url: &Url) -> Result<Url, IndexError> {
    if url.host_str() != Some("docs.google.com") {
        return Ok(url.clone());
    }
    let segments: Vec<&str> = url
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    let gid = url
        .query_pairs()
        .find(|(k, _)| k == "gid")
        .map(|(_, v)| v.into_owned());

    if segments.first() == Some(&"spreadsheets") && segments.get(1).copied() == Some("d") {
        if segments.get(2).copied() == Some("e") {
            if let Some(key) = segments.get(3).copied()
                && matches!(segments.get(4).copied(), Some("pubhtml" | "pub"))
            {
                let gid =
                    gid.ok_or_else(|| IndexError::InvalidUrl(format!("missing gid: {url}")))?;
                let out = if url.query_pairs().any(|(k, v)| k == "output" && v == "csv") {
                    url.to_string()
                } else {
                    format!(
                        "https://docs.google.com/spreadsheets/d/e/{key}/pub?gid={gid}&single=true&output=csv"
                    )
                };
                return Url::parse(&out).map_err(|e| IndexError::InvalidUrl(e.to_string()));
            }
        } else if let Some(key) = segments.get(2).copied() {
            // Only pin a gid when one is actually given (query param or
            // `#gid=` fragment). Defaulting to gid=0 breaks sheets whose first
            // tab isn't gid 0: `/export?format=csv&gid=0` returns HTTP 400,
            // while omitting gid lets the export default to the first visible
            // sheet and succeed.
            let gid = gid.or_else(|| {
                url.fragment()
                    .and_then(|f| f.strip_prefix("gid="))
                    .map(str::to_owned)
            });
            let out = gid.map_or_else(
                || format!("https://docs.google.com/spreadsheets/d/{key}/export?format=csv"),
                |gid| {
                    format!(
                        "https://docs.google.com/spreadsheets/d/{key}/export?format=csv&gid={gid}"
                    )
                },
            );
            return Url::parse(&out).map_err(|e| IndexError::InvalidUrl(e.to_string()));
        }
    }
    // Unknown docs.google.com path shapes (e.g. `/uc?id=...` direct downloads)
    // pass through unchanged; the HTTP fetch then either returns the bytes
    // verbatim or surfaces a meaningful status. Returning an error here would
    // block legitimate Google-hosted CSV URLs that don't live under
    // /spreadsheets/d/.
    Ok(url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn published_html_rewrites_to_pub_csv() {
        let inp = u("https://docs.google.com/spreadsheets/d/e/2PACX-x/pubhtml?gid=42&single=true");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(
            out.as_str(),
            "https://docs.google.com/spreadsheets/d/e/2PACX-x/pub?gid=42&single=true&output=csv"
        );
    }

    #[test]
    fn already_csv_pass_through() {
        let inp =
            u("https://docs.google.com/spreadsheets/d/e/2PACX-x/pub?gid=42&single=true&output=csv");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(out, inp);
    }

    #[test]
    fn edit_url_with_fragment_gid_rewrites_to_export() {
        let inp = u("https://docs.google.com/spreadsheets/d/KEY/edit#gid=7");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(
            out.as_str(),
            "https://docs.google.com/spreadsheets/d/KEY/export?format=csv&gid=7"
        );
    }

    #[test]
    fn edit_url_without_gid_omits_gid() {
        // No gid anywhere: must not pin gid=0 (some sheets' first tab isn't 0,
        // and `&gid=0` then 400s). Omitting gid lets Google pick the first tab.
        let inp = u("https://docs.google.com/spreadsheets/d/KEY/edit");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(
            out.as_str(),
            "https://docs.google.com/spreadsheets/d/KEY/export?format=csv"
        );
    }

    #[test]
    fn edit_url_with_query_gid_is_preserved() {
        let inp = u("https://docs.google.com/spreadsheets/d/KEY/edit?gid=42");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(
            out.as_str(),
            "https://docs.google.com/spreadsheets/d/KEY/export?format=csv&gid=42"
        );
    }

    #[test]
    fn non_google_url_passes_through() {
        let inp = u("https://example.org/foo.csv");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(out, inp);
    }

    #[test]
    fn unrecognized_google_url_passes_through() {
        let inp = u("https://docs.google.com/uc?id=ABC&export=download");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(out, inp);
    }
}
