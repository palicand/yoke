use anyhow::Error;
use thiserror::Error;

pub struct ExitInfo {
    pub code: i32,
    pub envelope_code: String,
    pub details: serde_json::Value,
}

/// User-input policy errors raised by the CLI itself.
///
/// Distinct from library-side errors so they can be mapped to exit 2 (usage) and
/// keep "internal" (exit 1) for genuinely unexpected failures.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("refusing to delete {name} without --force")]
    RequiresForce { name: String },
    #[error("--edits - and target - both consume stdin; pick one")]
    StdinConflict,
    #[error("unknown sub-profile mode: {value}")]
    UnknownMode { value: String },
    #[error("unknown channel: {value}")]
    UnknownChannel { value: String },
    #[error("malformed edits file: {message}")]
    MalformedEdits { message: String },
}

pub fn classify(err: &Error) -> ExitInfo {
    for cause in err.chain() {
        if let Some(ce) = cause.downcast_ref::<CliError>() {
            return classify_cli(ce);
        }
        if let Some(ve) = cause.downcast_ref::<yoke_volume::error::VolumeError>() {
            return classify_volume(ve);
        }
        if let Some(pe) = cause.downcast_ref::<yoke_config::ParseError>() {
            return ExitInfo {
                code: 4,
                envelope_code: "parse-error".into(),
                details: serde_json::json!({"message": pe.to_string()}),
            };
        }
        if let Some(ae) = cause.downcast_ref::<yoke_edit::ApplyError>() {
            return classify_edit(&ae.error, ae.index);
        }
        if let Some(ee) = cause.downcast_ref::<yoke_edit::EditError>() {
            return classify_edit(ee, 0);
        }
        if let Some(ie) = cause.downcast_ref::<yoke_index::IndexError>() {
            return classify_index(ie);
        }
    }
    ExitInfo {
        code: 1,
        envelope_code: "internal".into(),
        details: serde_json::json!({"message": err.to_string()}),
    }
}

fn classify_cli(err: &CliError) -> ExitInfo {
    let (code, envelope_code, details) = match err {
        CliError::RequiresForce { name } => {
            (2, "cli-requires-force", serde_json::json!({"name": name}))
        }
        CliError::StdinConflict => (2, "cli-stdin-conflict", serde_json::json!({})),
        CliError::UnknownMode { value } => {
            (2, "cli-unknown-mode", serde_json::json!({"value": value}))
        }
        CliError::UnknownChannel { value } => (
            2,
            "cli-unknown-channel",
            serde_json::json!({"value": value}),
        ),
        CliError::MalformedEdits { message } => (
            4,
            "cli-malformed-edits",
            serde_json::json!({"message": message}),
        ),
    };
    ExitInfo {
        code,
        envelope_code: envelope_code.into(),
        details,
    }
}

fn classify_volume(err: &yoke_volume::error::VolumeError) -> ExitInfo {
    use yoke_volume::error::VolumeError as V;
    match err {
        V::NotPresent => ExitInfo {
            code: 3,
            envelope_code: "not-present".into(),
            details: serde_json::json!({}),
        },
        V::VolumeHidden { hint } => ExitInfo {
            code: 3,
            envelope_code: "volume-hidden".into(),
            details: serde_json::json!({ "hint": hint }),
        },
        V::InvalidProfileName(s) => ExitInfo {
            code: 2,
            envelope_code: "invalid-name".into(),
            details: serde_json::json!({"name": s}),
        },
        V::Io(e) => ExitInfo {
            code: 6,
            envelope_code: "io".into(),
            details: serde_json::json!({"message": e.to_string()}),
        },
        V::BackendInit(s) => ExitInfo {
            code: 6,
            envelope_code: "backend-init".into(),
            details: serde_json::json!({"message": s}),
        },
    }
}

fn classify_edit(err: &yoke_edit::EditError, index: usize) -> ExitInfo {
    use yoke_edit::EditError as E;
    let (code, details) = match err {
        E::UnknownInput { input, suggestions } => (
            "edit-unknown-input",
            serde_json::json!({"input": input, "suggestions": suggestions, "index": index}),
        ),
        E::UnknownOutput {
            output,
            suggestions,
        } => (
            "edit-unknown-output",
            serde_json::json!({"output": output, "suggestions": suggestions, "index": index}),
        ),
        E::UnknownPreference { key, suggestions } => (
            "edit-unknown-preference",
            serde_json::json!({"key": key, "suggestions": suggestions, "index": index}),
        ),
        E::InvalidPreferenceValue {
            key,
            value,
            expected_type,
        } => (
            "edit-invalid-value",
            serde_json::json!({"key": key, "value": value, "expected_type": expected_type, "index": index}),
        ),
        E::SubProfileNotFound { name } => (
            "edit-subprofile-not-found",
            serde_json::json!({"name": name, "index": index}),
        ),
        E::SubProfileExists { name } => (
            "edit-subprofile-exists",
            serde_json::json!({"name": name, "index": index}),
        ),
        E::LastSubProfileDeletion => ("edit-last-subprofile", serde_json::json!({"index": index})),
    };
    ExitInfo {
        code: 5,
        envelope_code: code.into(),
        details,
    }
}

fn classify_index(err: &yoke_index::IndexError) -> ExitInfo {
    use yoke_index::IndexError as I;
    match err {
        I::Network(_) => ExitInfo {
            code: 7,
            envelope_code: "network".into(),
            details: serde_json::json!({"message": err.to_string()}),
        },
        I::FetchFailed { status, url } => ExitInfo {
            code: 7,
            envelope_code: "fetch-failed".into(),
            details: serde_json::json!({"status": status, "url": url.as_str()}),
        },
        I::InvalidUrl(s) => ExitInfo {
            code: 7,
            envelope_code: "invalid-url".into(),
            details: serde_json::json!({"message": s}),
        },
        I::NotFound(s) => ExitInfo {
            code: 7,
            envelope_code: "index-not-found".into(),
            details: serde_json::json!({"name": s}),
        },
        I::IndexFormat(s) => ExitInfo {
            code: 4,
            envelope_code: "index-format".into(),
            details: serde_json::json!({"message": s}),
        },
        I::NoCacheDir => ExitInfo {
            code: 6,
            envelope_code: "no-cache-dir".into(),
            details: serde_json::json!({"message": err.to_string()}),
        },
        I::Io(e) => ExitInfo {
            code: 6,
            envelope_code: "io".into(),
            details: serde_json::json!({"message": e.to_string()}),
        },
    }
}
