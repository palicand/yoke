mod common;
use common::yokectl;

#[test]
fn dynamic_completion_script_is_non_empty_for_each_shell() {
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let out = yokectl()
            .env("COMPLETE", shell)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(
            !out.is_empty(),
            "shell {shell} produced empty completion script"
        );
    }
}
