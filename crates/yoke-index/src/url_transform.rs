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
            let gid = gid.unwrap_or_else(|| {
                url.fragment()
                    .and_then(|f| f.strip_prefix("gid="))
                    .map_or_else(|| "0".into(), str::to_owned)
            });
            let out =
                format!("https://docs.google.com/spreadsheets/d/{key}/export?format=csv&gid={gid}");
            return Url::parse(&out).map_err(|e| IndexError::InvalidUrl(e.to_string()));
        }
    }
    Err(IndexError::InvalidUrl(format!(
        "unrecognized Google Sheets URL: {url}"
    )))
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
    fn non_google_url_passes_through() {
        let inp = u("https://example.org/foo.csv");
        let out = to_csv_export(&inp).unwrap();
        assert_eq!(out, inp);
    }
}
