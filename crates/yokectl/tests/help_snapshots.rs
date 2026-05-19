use assert_cmd::Command;

fn yokectl() -> Command {
    Command::cargo_bin("yokectl").unwrap()
}

fn help_for(args: &[&str]) -> String {
    let mut all = args.to_vec();
    all.push("--help");
    let out = yokectl()
        .env("COLUMNS", "100")
        .env("NO_COLOR", "1")
        .args(&all)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "help invocation failed for {args:?}:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

const CASES: &[(&str, &[&str])] = &[
    ("top_level", &[]),
    ("device", &["device"]),
    ("debug", &["debug"]),
    ("watch", &["watch"]),
    ("list", &["list"]),
    ("show", &["show"]),
    ("validate", &["validate"]),
    ("pull", &["pull"]),
    ("push", &["push"]),
    ("copy", &["copy"]),
    ("rename", &["rename"]),
    ("delete", &["delete"]),
    ("set_title", &["set-title"]),
    ("set_preference", &["set-preference"]),
    ("unset_preference", &["unset-preference"]),
    ("set_override", &["set-override"]),
    ("unset_override", &["unset-override"]),
    ("set_binding", &["set-binding"]),
    ("clear_binding", &["clear-binding"]),
    ("apply", &["apply"]),
    ("install", &["install"]),
    ("completions", &["completions"]),
    ("docs", &["docs"]),
    ("manual", &["manual"]),
    ("topic", &["topic"]),
    ("subprofile", &["subprofile"]),
    ("subprofile_add", &["subprofile", "add"]),
    ("subprofile_delete", &["subprofile", "delete"]),
    ("subprofile_rename", &["subprofile", "rename"]),
    ("subprofile_clone", &["subprofile", "clone"]),
    ("index", &["index"]),
    ("index_list", &["index", "list"]),
    ("index_search", &["index", "search"]),
    ("index_show", &["index", "show"]),
    ("index_update", &["index", "update"]),
    ("index_browse", &["index", "browse"]),
    ("catalog", &["catalog"]),
    ("catalog_inputs", &["catalog", "inputs"]),
    ("catalog_outputs", &["catalog", "outputs"]),
    ("catalog_preferences", &["catalog", "preferences"]),
    ("catalog_modes", &["catalog", "modes"]),
    ("catalog_channels", &["catalog", "channels"]),
];

#[test]
fn help_text_snapshots() {
    for (suffix, args) in CASES {
        let stdout = help_for(args);
        insta::with_settings!({ snapshot_suffix => (*suffix).to_string() }, {
            insta::assert_snapshot!(stdout);
        });
    }
}
