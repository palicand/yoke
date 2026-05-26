use super::*;

const SEED: &str =
    "QuadStick Configuration,Version 1.4,Mock,Default\r\n,,,\r\n*Main,sip_puff,,A\r\n";

#[test]
fn show_raw_output_is_captured_through_output() {
    let (_dir, provider) = seed_tempdir(&[("default.csv", SEED)]);
    let cli = Cli {
        fake_volume: None,
        json: false,
        verbose: 0,
        no_color: true,
        command: Commands::Show {
            target: "default".into(),
            raw: true,
        },
    };
    let cap = dispatch_in_process(cli, &provider);
    assert_eq!(cap.code, 0, "show --raw should succeed");
    assert_eq!(
        cap.stdout,
        SEED.as_bytes(),
        "raw show bytes were not captured through Output"
    );
}
