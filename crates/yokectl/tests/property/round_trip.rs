use super::*;

const SEED: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default\r\n,,,\r\n*Main,sip_puff,,A\r\n";

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn prop_round_trip_equality(
        actions in prop::collection::vec(action_strategy(&["default".into()]), 1..8)
    ) {
        let (dir, provider) = seed_tempdir(&[("default.csv", SEED)]);
        let base = Cli {
            fake_volume: None, json: false, verbose: 0, no_color: true,
            command: Commands::List,
        };
        for action in &actions {
            let cli = action_to_cli(action, &base);
            let _ = dispatch_in_process(cli, &provider);
        }
        // After every action sequence, every CSV in the tempdir must parse cleanly.
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let entry = entry.unwrap();
            if !entry.path().to_string_lossy().ends_with(".csv") { continue; }
            let bytes = std::fs::read(entry.path()).unwrap();
            let first = yoke_config::parse(&bytes).expect("first parse").model;
            let serialized = yoke_config::write(&first, None).expect("serialize");
            let second = yoke_config::parse(&serialized).expect("re-parse").model;
            prop_assert_eq!(first, second, "round-trip mismatch for {:?}", entry.path());
        }
    }
}
