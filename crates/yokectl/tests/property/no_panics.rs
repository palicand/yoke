use super::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, max_shrink_iters: 64, .. ProptestConfig::default() })]

    #[test]
    fn prop_no_panics(actions in prop::collection::vec(action_strategy(&["default".into()]), 1..16)) {
        let (_dir, provider) = seed_tempdir(&[("default.csv", SEED)]);
        let scratch = tempfile::tempdir().unwrap();
        let base = Cli {
            fake_volume: None,
            json: false,
            verbose: 0,
            no_color: true,
            command: Commands::List,
        };
        for action in &actions {
            let cli = action_to_cli(action, &base, scratch.path());
            let cap = dispatch_in_process(cli, &provider);
            // Exit codes must be in the documented set
            prop_assert!(matches!(cap.code, 0..=7),
                "unexpected exit code {} for action {action:?}", cap.code);
        }
    }
}
