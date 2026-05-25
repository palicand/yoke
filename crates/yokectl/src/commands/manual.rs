use anyhow::Result;
use serde_json::json;

use crate::commands::browser::{open_or_emit, unknown_slug};
use crate::output::Output;

const MANUAL_ROOT: &str = "https://quadstick.s3.amazonaws.com/documents/user_manual/um/";

// HEAD-checked 2026-05-19; sip-puff and modes have no standalone upstream page.
const MANUAL_TOPICS: &[(&str, &str)] = &[
    ("configuration", "configuration.htm"),
    ("google-sheets", "google_drive_spreadsheets.htm"),
    ("changing-profiles", "changing_profiles.htm"),
    ("dropdowns", "dropdown_lists_used_in_profiles.htm"),
    ("examples", "example_configuration_spreadsheets.htm"),
    ("preferences", "preferences.htm"),
    ("keyboard", "keyboard.htm"),
    ("mouse", "mouse.htm"),
    ("joystick", "joystick.htm"),
    ("reference-cards", "reference_cards.htm"),
    (
        "playstation-xbox-outputs",
        "selecting_output_names_for_playstation_and_xbox.htm",
    ),
];

pub fn run(out: &Output, topic: Option<&str>) -> Result<()> {
    match topic {
        // No slug → the listing is the discoverable surface in both formats.
        // Launching a browser without a slug would hide the topic list from
        // human-mode users and the listing's stable URLs from script-mode
        // users.
        None => emit_listing(out),
        Some(slug) => {
            let filename = MANUAL_TOPICS
                .iter()
                .find_map(|(s, f)| (*s == slug).then_some(*f))
                .ok_or_else(|| unknown_topic(slug))?;
            let url = format!("{MANUAL_ROOT}{filename}");
            open_or_emit(out, &json!({ "slug": slug, "url": url }), &url)
        }
    }
}

fn emit_listing(out: &Output) -> Result<()> {
    let topics: Vec<(&'static str, String)> = MANUAL_TOPICS
        .iter()
        .map(|(slug, file)| (*slug, format!("{MANUAL_ROOT}{file}")))
        .collect();
    let json_topics: Vec<_> = topics
        .iter()
        .map(|(slug, url)| json!({ "slug": slug, "url": url }))
        .collect();
    out.emit(&json!({ "root": root_url(), "topics": json_topics }), |w| {
        for (slug, url) in &topics {
            writeln!(w, "{slug:24}  {url}")?;
        }
        Ok(())
    })
}

fn root_url() -> String {
    format!("{MANUAL_ROOT}configuration.htm")
}

fn unknown_topic(slug: &str) -> anyhow::Error {
    let known: Vec<&str> = MANUAL_TOPICS.iter().map(|(s, _)| *s).collect();
    unknown_slug("manual topic", slug, &known)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics_are_unique() {
        let slugs: Vec<_> = MANUAL_TOPICS.iter().map(|(s, _)| *s).collect();
        let mut seen = std::collections::HashSet::new();
        for s in &slugs {
            assert!(seen.insert(*s), "duplicate slug: {s}");
        }
    }

    #[test]
    fn unknown_returns_suggestion_for_close_typo() {
        let e = unknown_topic("preferencs").to_string();
        assert!(e.contains("preferences"), "no suggestion in: {e}");
    }
}
