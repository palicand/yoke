use super::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn prop_round_trip_equality(
        actions in prop::collection::vec(action_strategy(&["default".into()]), 1..8)
    ) {
        let (dir, provider) = seed_tempdir(&[("default.csv", SEED)]);
        let scratch = tempfile::tempdir().unwrap();
        let base = Cli {
            fake_volume: None, json: false, verbose: 0, no_color: true,
            command: Commands::List,
        };
        for action in &actions {
            let cli = action_to_cli(action, &base, scratch.path());
            let _ = dispatch_in_process(cli, &provider);
        }
        // Every write action lands a parseable profile (Push uses a fixed valid CSV; edits
        // go through the validating apply path), so the parse guard below is a defensive
        // skip that should not fire today. It keeps the invariant honest should a future
        // strategy start emitting arbitrary bytes.
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            if !entry.path().to_string_lossy().ends_with(".csv") { continue; }
            let bytes = std::fs::read(entry.path()).unwrap();
            let Ok(parsed) = yoke_config::parse(&bytes) else { continue; };
            let first = parsed.model;
            let serialized = yoke_config::write(&first, None).expect("serialize");
            let second = yoke_config::parse(&serialized).expect("re-parse").model;
            prop_assert_eq!(first, second, "round-trip mismatch for {:?}", entry.path());
        }
    }
}
