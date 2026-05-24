// Each integration test binary compiles this module independently; symbols not
// used by a given binary trip dead_code. Suppress at module scope.
#![allow(dead_code)]

use assert_cmd::Command;

#[must_use]
pub fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
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
