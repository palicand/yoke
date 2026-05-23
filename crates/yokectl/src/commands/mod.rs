pub mod apply;
pub mod catalog;
pub mod device;
pub mod edit;
pub mod profile;
pub mod subprofile;

use yoke_volume::profile::ProfileEntry;

pub(crate) fn profile_entry_json(e: &ProfileEntry) -> serde_json::Value {
    serde_json::json!({
        "name": e.name.stem(),
        "kind": e.kind,
        "byte_len": e.byte_len,
    })
}
