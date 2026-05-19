use anyhow::{Result, anyhow};
use serde_json::json;

use crate::commands::browser::{closest, open_or_emit};
use crate::output::Output;

const MANUAL_ROOT: &str = "https://quadstick.s3.amazonaws.com/documents/user_manual/um/";

// Slug → upstream filename under MANUAL_ROOT. Each entry was HEAD-checked
// against the live site on 2026-05-19; sip-puff and modes do not exist as
// standalone pages and so are not represented here.
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
        None if out.is_json() => emit_listing(out),
        None => open_or_emit(out, &json!({ "url": root_url() }), &root_url()),
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
    let topics: Vec<_> = MANUAL_TOPICS
        .iter()
        .map(|(slug, file)| json!({ "slug": slug, "url": format!("{MANUAL_ROOT}{file}") }))
        .collect();
    out.emit(&json!({ "root": root_url(), "topics": topics }), |_| Ok(()))
}

fn root_url() -> String {
    format!("{MANUAL_ROOT}configuration.htm")
}

fn unknown_topic(slug: &str) -> anyhow::Error {
    let known: Vec<&str> = MANUAL_TOPICS.iter().map(|(s, _)| *s).collect();
    let suggestions = closest(slug, &known);
    let suggestions_str = if suggestions.is_empty() {
        format!("available: {known:?}")
    } else {
        format!("did you mean: {suggestions:?}")
    };
    anyhow!(format!("unknown manual topic: {slug:?}; {suggestions_str}"))
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
