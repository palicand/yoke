use super::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn prop_exit_code_and_json_shape_consistent(
        action in action_strategy(&["default".into()])
    ) {
        let (_dir1, provider1) = seed_tempdir(&[("default.csv", SEED)]);
        let (_dir2, provider2) = seed_tempdir(&[("default.csv", SEED)]);
        let scratch = tempfile::tempdir().unwrap();
        let base = Cli { fake_volume: None, json: false, verbose: 0, no_color: true, command: Commands::List };
        let cli_human = action_to_cli(&action, &base, scratch.path());
        let mut cli_json = cli_human.clone();
        cli_json.json = true;
        let human = dispatch_in_process(cli_human, &provider1);
        let json = dispatch_in_process(cli_json, &provider2);
        prop_assert_eq!(human.code, json.code, "exit code diverged between human and json runs");
        prop_assert!(matches!(human.code, 0..=7));
        // `show --raw` deliberately emits the profile's raw CSV bytes, not JSON, so it is
        // exempt from the shape check. Every other command must produce a single JSON
        // document on stdout under --json (or nothing for commands that print none).
        let raw_show = matches!(action, Action::Show { raw: true, .. });
        if !raw_show && !json.stdout.is_empty() {
            let _: serde_json::Value = serde_json::from_slice(&json.stdout)
                .map_err(|e| TestCaseError::fail(format!("stdout not valid JSON for {action:?}: {e}")))?;
        }
    }
}
