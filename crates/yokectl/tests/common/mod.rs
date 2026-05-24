// Each integration test binary compiles this module independently; symbols not
// used by a given binary trip dead_code. Suppress at module scope.
#![allow(dead_code)]

use assert_cmd::Command;

/// `assert_cmd::Command` with the install-stack env vars pre-scrubbed.
///
/// `YOKECTL_INDEX_URL` and `YOKECTL_CACHE_DIR` are inherited from the
/// developer's shell otherwise: a stale URL would silently redirect a test
/// to the real community sheet, and a stale cache dir lets one test's
/// fixture surface in another.
#[must_use]
pub fn yokectl() -> Command {
    let mut cmd = Command::cargo_bin("yokectl").unwrap();
    cmd.env_remove("YOKECTL_INDEX_URL")
        .env_remove("YOKECTL_CACHE_DIR");
    cmd
}

pub fn seed_profile(dir: &std::path::Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
}

pub const FIXTURE: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default,,\n,,,,\n*Main,sip_puff,,A,inputs\n";

pub const FIXTURE_WITH_SUB: &str = "QuadStick Configuration,Version 1.4,Mock,Default\r\n\
Profile Name,Main,Mouse,usb\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n";
