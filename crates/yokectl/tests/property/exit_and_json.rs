use super::*;

const SEED: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default\r\n,,,\r\n*Main,sip_puff,,A\r\n";

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, .. ProptestConfig::default() })]

    #[test]
    fn prop_exit_code_and_json_shape_consistent(
        action in action_strategy(&["default".into()])
    ) {
        let (_dir1, provider1) = seed_tempdir(&[("default.csv", SEED)]);
        let (_dir2, provider2) = seed_tempdir(&[("default.csv", SEED)]);
        let base = Cli { fake_volume: None, json: false, verbose: 0, no_color: true, command: Commands::List };
        let cli_human = action_to_cli(&action, &base);
        let mut cli_json = cli_human.clone();
        cli_json.json = true;
        let human = dispatch_in_process(cli_human, &provider1);
        let json = dispatch_in_process(cli_json, &provider2);
        prop_assert_eq!(human.code, json.code, "exit code diverged between human and json runs");
        prop_assert!(matches!(human.code, 0..=7));
        // For JSON, stdout must parse as a single JSON document (or be empty for commands that produce none).
        if !json.stdout.is_empty() {
            let _: serde_json::Value = serde_json::from_slice(&json.stdout)
                .map_err(|e| TestCaseError::fail(format!("stdout not valid JSON for {action:?}: {e}")))?;
        }
    }
}
