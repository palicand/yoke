use std::io::Write;

use anyhow::{Result, anyhow};
use serde_json::json;

use crate::commands::browser::closest;
use crate::output::Output;

const TOPICS: &[(&str, &str)] = &[
    (
        "binding-model",
        include_str!("../../topics/binding-model.md"),
    ),
    ("sub-profiles", include_str!("../../topics/sub-profiles.md")),
    ("sip-puff", include_str!("../../topics/sip-puff.md")),
    ("preferences", include_str!("../../topics/preferences.md")),
    (
        "install-sources",
        include_str!("../../topics/install-sources.md"),
    ),
];

pub fn run(out: &Output, name: Option<&str>) -> Result<()> {
    name.map_or_else(|| emit_listing(out), |slug| emit_topic(out, slug))
}

fn emit_listing(out: &Output) -> Result<()> {
    let topics: Vec<_> = TOPICS
        .iter()
        .map(|(slug, body)| json!({ "slug": slug, "title": title_of(body) }))
        .collect();
    out.emit(&json!({ "topics": topics }), |w| {
        for (slug, body) in TOPICS {
            writeln!(w, "{:18}  {}", slug, title_of(body))?;
        }
        Ok(())
    })
}

fn emit_topic(out: &Output, slug: &str) -> Result<()> {
    let body = TOPICS
        .iter()
        .find_map(|(s, b)| (*s == slug).then_some(*b))
        .ok_or_else(|| unknown_topic(slug))?;
    out.emit(
        &json!({ "slug": slug, "title": title_of(body), "body": body }),
        |w| w.write_all(body.as_bytes()),
    )
}

fn title_of(body: &str) -> &str {
    body.lines()
        .next()
        .and_then(|l| l.strip_prefix("# "))
        .unwrap_or("")
}

fn unknown_topic(slug: &str) -> anyhow::Error {
    let known: Vec<&str> = TOPICS.iter().map(|(s, _)| *s).collect();
    let suggestions = closest(slug, &known);
    let suggestions_str = if suggestions.is_empty() {
        format!("available: {known:?}")
    } else {
        format!("did you mean: {suggestions:?}")
    };
    anyhow!(format!("unknown topic: {slug:?}; {suggestions_str}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_topic_has_a_markdown_title() {
        for (slug, body) in TOPICS {
            let first = body.lines().next().unwrap_or("");
            assert!(
                first.starts_with("# "),
                "topic {slug}: first line is not '# ...': {first:?}"
            );
        }
    }

    #[test]
    fn every_topic_is_at_least_twenty_lines() {
        for (slug, body) in TOPICS {
            let count = body.lines().count();
            assert!(count >= 20, "topic {slug}: only {count} lines");
        }
    }

    #[test]
    fn slugs_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for (s, _) in TOPICS {
            assert!(seen.insert(*s), "duplicate topic slug: {s}");
        }
    }

    #[test]
    fn unknown_returns_close_suggestion() {
        let e = unknown_topic("bindng-model").to_string();
        assert!(e.contains("binding-model"), "no suggestion in: {e}");
    }
}
