#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use yoke_ui::backend::Backend;
use yoke_ui::backend::mock::MockBackend;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn mock_backend_parses_fixture() {
    let backend = MockBackend::new().expect("mock fixture parses");
    let _: Arc<dyn Backend> = Arc::new(backend);
}

#[wasm_bindgen_test]
async fn mock_lists_one_device_profile() {
    let backend: Arc<dyn Backend> = Arc::new(MockBackend::new().expect("mock fixture parses"));
    let entries = backend
        .list_device_profiles()
        .await
        .expect("mock backend returns device profiles");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "default.csv");
}

#[wasm_bindgen_test]
async fn mock_returns_profile_for_device_open() {
    let backend: Arc<dyn Backend> = Arc::new(MockBackend::new().expect("mock fixture parses"));
    let profile = backend
        .read_device_profile("default.csv".into())
        .await
        .expect("mock backend returns a profile");
    assert!(!profile.sub_profiles.is_empty());
}
