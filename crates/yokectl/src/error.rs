use anyhow::Error;

pub struct ExitInfo {
    pub code: i32,
    pub envelope_code: String,
    pub details: serde_json::Value,
}

pub fn classify(err: &Error) -> ExitInfo {
    for cause in err.chain() {
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
