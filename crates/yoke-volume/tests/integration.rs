use std::time::Duration;
use tempfile::tempdir;
use yoke_volume::state::MountEvent;
use yoke_volume::{FsBackend, ProfileKind, ProfileName, VolumeError, VolumeProvider};

const FIXTURE_CSV: &[u8] = b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n";

fn pname(s: &str) -> ProfileName {
    ProfileName::new(s).unwrap()
}

#[test]
fn byte_level_round_trip_via_fs_backend() {
    let dir = tempdir().unwrap();
    let backend = FsBackend::new(dir.path().to_path_buf());
    backend
        .write_profile(&pname("sample"), FIXTURE_CSV)
        .unwrap();
    let read = backend.read_profile(&pname("sample")).unwrap();
    assert_eq!(read, FIXTURE_CSV, "round-trip bytes must match input");
}

#[test]
fn model_level_round_trip_via_yoke_config() {
    let dir = tempdir().unwrap();
    let backend = FsBackend::new(dir.path().to_path_buf());

    let parsed_before = yoke_config::parse(FIXTURE_CSV).expect("fixture must parse");

    backend
        .write_profile(&pname("sample"), FIXTURE_CSV)
        .unwrap();
    let on_disk = backend.read_profile(&pname("sample")).unwrap();

    let parsed_after = yoke_config::parse(&on_disk).expect("round-tripped fixture must parse");

    assert_eq!(parsed_before.model, parsed_after.model);
}

#[test]
fn multi_profile_lifecycle() {
    let dir = tempdir().unwrap();
    let backend = FsBackend::new(dir.path().to_path_buf());

    backend
        .write_profile(&pname("default"), FIXTURE_CSV)
        .unwrap();
    backend
        .write_profile(&pname("destiny"), FIXTURE_CSV)
        .unwrap();
    backend
        .write_profile(&pname("forza5"), FIXTURE_CSV)
        .unwrap();

    let entries = backend.list_profiles().unwrap();
    assert_eq!(entries.len(), 3);
    let kinds: std::collections::HashMap<_, _> = entries
        .iter()
        .map(|e| (e.name.as_filename().to_string(), e.kind))
        .collect();
    assert_eq!(kinds["default.csv"], ProfileKind::Default);
    assert_eq!(kinds["destiny.csv"], ProfileKind::Game);
    assert_eq!(kinds["forza5.csv"], ProfileKind::Game);

    backend
        .rename_profile(&pname("destiny"), &pname("destiny2"))
        .unwrap();
    let after_rename: Vec<_> = backend
        .list_profiles()
        .unwrap()
        .into_iter()
        .map(|e| e.name.as_filename().to_string())
        .collect();
    assert!(after_rename.contains(&"destiny2.csv".to_string()));
    assert!(!after_rename.contains(&"destiny.csv".to_string()));

    backend.delete_profile(&pname("forza5")).unwrap();
    let after_delete = backend.list_profiles().unwrap();
    assert_eq!(after_delete.len(), 2);
    let read_deleted = backend.read_profile(&pname("forza5"));
    assert!(matches!(
        read_deleted,
        Err(VolumeError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn not_present_failure_path() {
    let dir = tempdir().unwrap();
    let backend = FsBackend::new(dir.path().to_path_buf());
    backend.write_profile(&pname("x"), b"first").unwrap();

    backend.set_present(false);

    assert!(matches!(
        backend.list_profiles(),
        Err(VolumeError::NotPresent)
    ));
    assert!(matches!(
        backend.read_profile(&pname("x")),
        Err(VolumeError::NotPresent)
    ));
    assert!(matches!(
        backend.write_profile(&pname("x"), b"data"),
        Err(VolumeError::NotPresent)
    ));
    assert!(matches!(
        backend.delete_profile(&pname("x")),
        Err(VolumeError::NotPresent)
    ));

    backend.set_present(true);

    assert!(backend.read_profile(&pname("x")).is_ok());
}

#[test]
fn event_stream_observation() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();

    rt.block_on(async {
        let dir = tempdir().unwrap();
        let backend = FsBackend::new(dir.path().to_path_buf());
        let mut events = backend.subscribe_events();

        backend.set_present(false);
        backend.set_present(true);

        let timeout = Duration::from_millis(100);

        let first = tokio::time::timeout(timeout, events.recv())
            .await
            .expect("first event must arrive within 100ms")
            .expect("event channel must not be closed or lagged");
        assert!(matches!(first, MountEvent::VolumeUnmounted));

        let second = tokio::time::timeout(timeout, events.recv())
            .await
            .expect("second event must arrive within 100ms")
            .expect("event channel must not be closed or lagged");
        assert!(matches!(second, MountEvent::VolumeMounted { .. }));
    });
}
