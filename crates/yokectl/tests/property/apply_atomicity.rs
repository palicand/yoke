use super::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    #[test]
    fn prop_apply_atomicity_keeps_file_byte_identical(
        good_count in 1usize..4,
    ) {
        let (dir, provider) = seed_tempdir(&[("default.csv", SEED)]);
        let target = dir.path().join("default.csv");
        let pre = std::fs::read(&target).unwrap();
        let mut ops: Vec<EditOp> = (0..good_count)
            .map(|_| EditOp::SetTitle { title: "A".into() })
            .collect();
        ops.push(EditOp::AddBinding {
            sub_profile: "DoesNotExist".into(),
            input: "lip".into(),
            output: "touch".into(),
            modifier: None,
        });
        let edits_path = dir.path().join("edits.json");
        let envelope = serde_json::json!({ "edits": ops });
        std::fs::write(&edits_path, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();
        let cli = Cli {
            fake_volume: None,
            json: true,
            verbose: 0,
            no_color: true,
            command: Commands::Apply {
                target: "default".into(),
                edits: edits_path,
                dry_run: false,
            },
        };
        let cap = dispatch_in_process(cli, &provider);
        prop_assert_eq!(cap.code, 5, "expected edit exit code 5, got {}", cap.code);
        let post = std::fs::read(&target).unwrap();
        prop_assert_eq!(pre, post, "file was mutated despite all-or-nothing apply");
    }
}
