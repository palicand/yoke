pub mod apply;
pub(crate) mod browser;
pub mod catalog;
pub mod device;
pub mod edit;
pub mod index;
pub mod install;
pub mod manual;
pub mod profile;
pub mod subprofile;
pub mod topic;

use yoke_volume::profile::ProfileEntry;

pub(crate) fn profile_entry_json(e: &ProfileEntry) -> serde_json::Value {
    serde_json::json!({
        "name": e.name.stem(),
        "kind": e.kind,
        "byte_len": e.byte_len,
    })
}
