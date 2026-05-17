use yoke_config::{parse, write};

fn assert_byte_round_trip(label: &str, input: &[u8]) {
    let r = parse(input).unwrap_or_else(|e| panic!("[{label}] parse failed: {e}"));
    let out =
        write(&r.model, Some(&r.raw)).unwrap_or_else(|e| panic!("[{label}] write failed: {e}"));
    pretty_assertions::assert_eq!(
        std::str::from_utf8(&out).expect("utf8"),
        std::str::from_utf8(input).expect("utf8"),
        "byte round-trip failed for [{label}]",
    );
}

#[test]
fn single_sub_profile_with_delay_arg() {
    assert_byte_round_trip(
        "single+delay",
        b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n",
    );
}

#[test]
fn multi_sub_profile_with_mode_synonyms() {
    assert_byte_round_trip(
        "multi",
        b"QuadStick Configuration,Version 1.4,abc,Mac\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n\
Profile Name,,Left joy,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
left_joy_up,normal,up,\r\n\
\r\n",
    );
}

#[test]
fn delay_on_without_arg() {
    assert_byte_round_trip(
        "delay-no-arg",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left_button,delay_on,mp_center_puff,\r\n\
\r\n",
    );
}

#[test]
fn preference_override_inside_sub_profile() {
    assert_byte_round_trip(
        "pref-override",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
joystick_dead_zone_shape,normal,1,\r\n\
\r\n",
    );
}

#[test]
fn interleaved_bindings_and_overrides_preserve_order() {
    // binding, override, binding, override — the writer must emit them in
    // source order, not group all bindings first.
    assert_byte_round_trip(
        "interleaved",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
joystick_dead_zone_shape,normal,1,\r\n\
mouse_right,normal,right,\r\n\
anti_dead_zone,normal,5,\r\n\
\r\n",
    );
}

#[test]
fn comments_in_column_k() {
    assert_byte_round_trip(
        "comments",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,,,,,,,,Comments\r\n\
select,normal,mp_left_sip_soft,,,,,,,,Share or Create\r\n\
\r\n",
    );
}

#[test]
fn standalone_prefs_csv() {
    assert_byte_round_trip(
        "prefs",
        b"QuadStick Configuration,Version 1.1\r\n\
Preferences,,,,\r\n\
prefs.csv,,,,\r\n\
Preference,Value,Units,Description,\r\n\
volume,40,,,\r\n\
brightness,75,,,\r\n\
\r\n",
    );
}

#[test]
fn bluetooth_channel_section() {
    assert_byte_round_trip(
        "bluetooth",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,Bluetooth,\r\n\
mouse_left,normal,left,\r\n\
\r\n",
    );
}

#[test]
fn unknown_vocabulary_round_trips() {
    assert_byte_round_trip(
        "unknown",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mystery_output,future_modifier 5 7,unknown_input,\r\n\
\r\n",
    );
}

#[test]
fn infrared_section_round_trips_opaque() {
    assert_byte_round_trip(
        "infrared",
        b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
\r\n\
Infrared,,,,\r\n\
ir_code,38KHz,0x01ABCDEF,,,\r\n\
\r\n",
    );
}

#[test]
fn warnings_are_emitted_for_unknowns() {
    let input: &[u8] = b"QuadStick Configuration,Version 1.4,,Test\r\n\
Profile Name,,Mouse,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mystery_output,future_mod,mp_center_puff,\r\n\
\r\n";
    let r = parse(input).unwrap();
    let unknown_output = r.warnings.iter().any(|w| {
        matches!(w,
        yoke_config::Warning::UnknownOutput { id, .. } if id == "mystery_output")
    });
    let unknown_mod = r.warnings.iter().any(|w| {
        matches!(w,
        yoke_config::Warning::UnknownModifier { name, .. } if name == "future_mod")
    });
    assert!(unknown_output, "expected UnknownOutput warning");
    assert!(unknown_mod, "expected UnknownModifier warning");
}

#[test]
fn parse_rejects_non_utf8() {
    let bad: &[u8] = b"\xff\xfe not utf8\r\n";
    match parse(bad) {
        Err(yoke_config::ParseError::Encoding) => (),
        other => panic!("expected Encoding, got {other:?}"),
    }
}
