use yoke_config::catalog::{
    Channel, DPadDir, GamepadButton, JoyAxis, JoyOutput, KbKey, MouseAction, MpPosition,
    PreferenceSpec, PreferenceValueKind, SipPuff, SubProfileMode, SystemAction, UsbHost,
};
use yoke_config::model::{Profile, SubProfile, SubProfileHeader, TopLine};
use yoke_edit::{EditOp, PreferenceValue, apply};

fn empty_profile_with_main() -> Profile {
    Profile {
        top_line: TopLine {
            label: "QuadStick Configuration".into(),
            version: "Version 1.4".into(),
            source: String::new(),
            title: "Default".into(),
            trailing_cells: vec![],
            width: 4,
        },
        sub_profiles: vec![SubProfile {
            header: SubProfileHeader {
                profile_name: "Main".into(),
                mode: SubProfileMode::Mouse,
                sub_mode: String::new(),
                channel: Channel::Usb,
                column_header_label: String::new(),
            },
            rows: vec![],
        }],
        preferences: None,
        infrared: vec![],
    }
}

fn assert_input_accepted(p: &Profile, input: &str) {
    let r = apply(
        p.clone(),
        &[EditOp::AddBinding {
            sub_profile: 0,
            input: input.into(),
            output: "kb_a".into(),
            modifier: None,
        }],
    );
    assert!(r.is_ok(), "input {input:?} rejected unexpectedly: {r:?}");
}

fn assert_output_accepted(p: &Profile, output: &str) {
    let r = apply(
        p.clone(),
        &[EditOp::AddBinding {
            sub_profile: 0,
            input: "lip".into(),
            output: output.into(),
            modifier: None,
        }],
    );
    assert!(r.is_ok(), "output {output:?} rejected unexpectedly: {r:?}");
}

#[test]
fn every_input_catalog_variant_is_accepted_by_add_binding() {
    let p = empty_profile_with_main();
    // Mouthpiece + sip/puff + mp_position combinations exercise SipPuff::ALL and MpPosition::ALL.
    for sp in SipPuff::ALL {
        for pos in MpPosition::ALL {
            assert_input_accepted(&p, &format!("mp_{}_{}", pos.as_csv(), sp.as_csv()));
            assert_input_accepted(&p, &format!("mp_{}_{}_soft", pos.as_csv(), sp.as_csv()));
        }
        // Side tube (covers SideKind::Hard/Soft/Long via the `right_*` family)
        assert_input_accepted(&p, &format!("right_{}", sp.as_csv()));
        assert_input_accepted(&p, &format!("right_{}_soft", sp.as_csv()));
        assert_input_accepted(&p, &format!("right_{}_long", sp.as_csv()));
    }
    assert_input_accepted(&p, "lip");
    assert_input_accepted(&p, "lip_soft");

    // Joystick analog axes (covers JoyAxis::ALL).
    for ax in JoyAxis::ALL {
        assert_input_accepted(&p, ax.as_csv());
    }
    assert_input_accepted(&p, "any_direction");
    assert_input_accepted(&p, "center");
    assert_input_accepted(&p, "constant");

    // Joystick D-pad outer + inner zones (covers DPadDir::ALL).
    for d in DPadDir::ALL {
        assert_input_accepted(&p, d.as_csv());
        assert_input_accepted(&p, &format!("{}_inner", d.as_csv()));
    }

    // USB-A host axes/dpads/buttons (covers UsbHost::ALL).
    for host in UsbHost::ALL {
        let h = host.as_csv_index();
        for ax in JoyAxis::ALL {
            assert_input_accepted(&p, &format!("usb_{h}_{}", ax.as_csv()));
        }
        for d in DPadDir::ALL {
            assert_input_accepted(&p, &format!("usb_{h}_{}", d.as_csv()));
            assert_input_accepted(&p, &format!("usb_{h}_{}_inner", d.as_csv()));
        }
        for n in 1u8..=15 {
            assert_input_accepted(&p, &format!("usb_{h}_button_{n}"));
        }
    }

    // Digital inputs.
    for n in 1u8..=8 {
        assert_input_accepted(&p, &format!("digital_in_{n}"));
    }
}

#[test]
fn every_output_catalog_variant_is_accepted_by_add_binding() {
    let p = empty_profile_with_main();
    for k in KbKey::ALL {
        assert_output_accepted(&p, k.as_csv());
    }
    for m in MouseAction::ALL {
        assert_output_accepted(&p, m.as_csv());
    }
    for g in GamepadButton::ALL {
        assert_output_accepted(&p, g.as_csv());
    }
    for d in DPadDir::ALL {
        assert_output_accepted(&p, &format!("dpad_{}", d.as_csv()));
    }
    for j in JoyOutput::ALL {
        assert_output_accepted(&p, j.as_csv());
    }
    for s in SystemAction::ALL {
        assert_output_accepted(&p, s.as_csv());
    }
    assert_output_accepted(&p, "touch");
}

#[test]
fn every_known_preference_is_accepted_by_set_preference() {
    let p = empty_profile_with_main();
    for spec in PreferenceSpec::ALL {
        let value = match spec.kind {
            PreferenceValueKind::IntRange { min, .. } => PreferenceValue::Number(i64::from(min)),
            PreferenceValueKind::SelectInt(opts) => PreferenceValue::Number(i64::from(opts[0])),
            PreferenceValueKind::Bool => PreferenceValue::Bool(false),
            PreferenceValueKind::Select(opts) => PreferenceValue::Text((*opts[0]).to_owned()),
            PreferenceValueKind::Text => PreferenceValue::Text(String::new()),
        };
        let r = apply(
            p.clone(),
            &[EditOp::SetPreference {
                key: spec.id.to_owned(),
                value,
            }],
        );
        assert!(r.is_ok(), "preference {} rejected: {r:?}", spec.id);
    }
}
