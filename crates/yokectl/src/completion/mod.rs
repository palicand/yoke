//! Completion-path helpers.
//!
//! Every function here MUST be silent on failure — return an empty `Vec`
//! instead of erroring, and never write to stderr — because the shell calls
//! into us mid-type and any output corrupts the prompt.

pub mod profile;
pub mod subprofile;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use yoke_volume::VolumeProvider;

const PROBE_TIMEOUT: Duration = Duration::from_millis(200);

/// Builds a `VolumeProvider` from `argv` for the completion path.
///
/// Honours `--fake-volume <path>` if it appears earlier in the line;
/// otherwise tries the platform backend with a hard 200 ms budget.
/// Returns `None` on any failure — callers degrade to an empty candidate list.
#[must_use]
pub fn resolve_backend_for_completion(
    argv: &[std::ffi::OsString],
) -> Option<Arc<dyn VolumeProvider>> {
    if let Some(path) = fake_volume_from_argv(argv) {
        return Some(
            Arc::new(yoke_volume::fs_backend::FsBackend::new(path)) as Arc<dyn VolumeProvider>
        );
    }
    platform_backend_with_timeout()
}

fn fake_volume_from_argv(argv: &[std::ffi::OsString]) -> Option<PathBuf> {
    let mut iter = argv.iter();
    while let Some(arg) = iter.next() {
        let s = arg.to_string_lossy();
        if s == "--fake-volume" {
            return iter.next().map(PathBuf::from);
        }
        if let Some(rest) = s.strip_prefix("--fake-volume=") {
            return Some(PathBuf::from(rest));
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn platform_backend_with_timeout() -> Option<Arc<dyn VolumeProvider>> {
    use std::sync::mpsc;
    use std::thread;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = yoke_volume_macos::MacOsVolumeProvider::new()
            .ok()
            .map(|b| Arc::new(b) as Arc<dyn VolumeProvider>);
        let _ = tx.send(result);
    });
    rx.recv_timeout(PROBE_TIMEOUT).ok().flatten()
}

#[cfg(not(target_os = "macos"))]
fn platform_backend_with_timeout() -> Option<Arc<dyn VolumeProvider>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn fake_volume_from_separate_argv() {
        let argv = [OsString::from("--fake-volume"), OsString::from("/tmp/x")];
        assert_eq!(fake_volume_from_argv(&argv), Some(PathBuf::from("/tmp/x")));
    }

    #[test]
    fn fake_volume_from_equals_form() {
        let argv = [OsString::from("--fake-volume=/tmp/y")];
        assert_eq!(fake_volume_from_argv(&argv), Some(PathBuf::from("/tmp/y")));
    }

    #[test]
    fn absent_when_flag_missing() {
        let argv = [OsString::from("show"), OsString::from("default")];
        assert_eq!(fake_volume_from_argv(&argv), None);
    }
}
