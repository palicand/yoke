use crate::app::{PickerState, PickerTarget};
use crate::edit::{CATEGORIES, output_category};
use crate::theme::Palette;
use yoke_config::catalog::outputs::Output;

pub enum PickerOutcome {
    Open,
    Close,
    /// csv id of the chosen output (Add / `EditOutput` targets).
    CommitOutput(String),
    /// composed modifier csv (`EditModifier` target).
    CommitModifier(String),
}

pub fn show(
    ctx: &egui::Context,
    state: &mut PickerState,
    palette: &Palette,
    // duplicate pre-check, evaluated against current(): (modifier csv) -> exists
    has_modifier: &dyn Fn(&str) -> bool,
) -> PickerOutcome {
    let mut outcome = PickerOutcome::Open;
    let modal = egui::Modal::new(egui::Id::new("yoke_picker")).show(ctx, |ui| {
        ui.set_width(440.0);
        match &state.target {
            PickerTarget::EditModifier { .. } => {
                outcome = modifier_body(ui, state, palette, has_modifier);
            }
            PickerTarget::AddBinding { .. } | PickerTarget::EditOutput { .. } => {
                outcome = output_body(ui, state, palette, has_modifier);
            }
        }
    });
    if modal.should_close() && matches!(outcome, PickerOutcome::Open) {
        return PickerOutcome::Close;
    }
    outcome
}

fn output_body(
    ui: &mut egui::Ui,
    state: &mut PickerState,
    palette: &Palette,
    has_modifier: &dyn Fn(&str) -> bool,
) -> PickerOutcome {
    let title = match &state.target {
        PickerTarget::AddBinding { input } => format!("Bind {input}"),
        PickerTarget::EditOutput { input, modifier } => {
            format!("Change output for {input} [{modifier}]")
        }
        PickerTarget::EditModifier { .. } => unreachable!("routed to modifier_body"),
    };
    ui.heading(title);

    // Key-capture banner (output mode only, per the handoff).
    ui.horizontal(|ui| {
        if state.capture_armed {
            ui.colored_label(palette.accent, "Press a key...");
        } else if ui.button("Bind a key directly").clicked() {
            state.capture_armed = true;
            state.capture_error = None;
        }
        if let Some(err) = &state.capture_error {
            ui.colored_label(palette.system, err);
        }
    });
    if state.capture_armed {
        let key = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key, pressed: true, ..
                } => Some(*key),
                _ => None,
            })
        });
        if let Some(k) = key {
            state.capture_armed = false;
            match key_to_output_id(k) {
                Some(id) => {
                    // Capture path for AddBinding: must pass the same modifier
                    // gate as the list-pick path. Skipping it would let a
                    // half-filled or duplicate modifier bypass the pre-check,
                    // which would then panic commit_output's .expect or
                    // silently double-key the input.
                    if let Some(outcome) = gated_commit(state, has_modifier, id.to_owned()) {
                        return outcome;
                    }
                }
                None => state.capture_error = Some(format!("No output for {k:?}")),
            }
        }
    }

    ui.add_space(6.0);
    ui.text_edit_singleline(&mut state.search);
    ui.horizontal_wrapped(|ui| {
        for cat in CATEGORIES {
            let selected = state.category == Some(*cat);
            if ui.selectable_label(selected, *cat).clicked() {
                state.category = if selected { None } else { Some(cat) };
            }
        }
    });
    ui.separator();

    // Modifier sub-control on add: the new chord's key (default normal).
    if matches!(state.target, PickerTarget::AddBinding { .. }) {
        modifier_subcontrol(ui, state, palette);
    }

    let needle = state.search.to_lowercase();
    let mut picked: Option<String> = None;
    egui::ScrollArea::vertical()
        .max_height(320.0)
        .show(ui, |ui| {
            for output in Output::iter_known() {
                let cat = output_category(&output);
                if state.category.is_some_and(|c| c != cat) {
                    continue;
                }
                let id = output.to_csv();
                if !needle.is_empty() && !id.to_lowercase().contains(&needle) {
                    continue;
                }
                if ui
                    .selectable_label(false, format!("{id}  ({cat})"))
                    .clicked()
                {
                    picked = Some(id);
                }
            }
        });

    if let Some(id) = picked
        && let Some(outcome) = gated_commit(state, has_modifier, id)
    {
        return outcome;
    }
    PickerOutcome::Open
}

/// Add-mode duplicate pre-check shared by the key-capture and list-pick
/// commit paths: the composed modifier must not already key a row on this
/// input (`BindingExists` pre-empted). Returns `None` after storing the
/// error so the picker stays open.
fn gated_commit(
    state: &mut PickerState,
    has_modifier: &dyn Fn(&str) -> bool,
    id: String,
) -> Option<PickerOutcome> {
    if !matches!(state.target, PickerTarget::AddBinding { .. }) {
        return Some(PickerOutcome::CommitOutput(id));
    }
    match crate::edit::compose_modifier(&state.keyword, &state.args) {
        Ok(m) if has_modifier(&m) => {
            state.capture_error = Some(format!("{m} already bound on this input"));
            None
        }
        Ok(_) => Some(PickerOutcome::CommitOutput(id)),
        Err(e) => {
            state.capture_error = Some(e);
            None
        }
    }
}

/// Shared modifier field editor: keyword selector + positional arg inputs.
/// Returns the composed modifier csv on success, or an error string.
fn modifier_fields(
    ui: &mut egui::Ui,
    state: &mut PickerState,
    palette: &Palette,
) -> Result<String, String> {
    ui.horizontal_wrapped(|ui| {
        for kw in yoke_config::catalog::Modifier::KEYWORDS {
            let selected = state.keyword == *kw;
            if ui.selectable_label(selected, *kw).clicked() && !selected {
                (*kw).clone_into(&mut state.keyword);
                state.args = vec![String::new(); crate::edit::modifier_arg_labels(kw).len()];
            }
        }
    });
    let labels = crate::edit::modifier_arg_labels(&state.keyword);
    state.args.resize(labels.len(), String::new());
    ui.horizontal(|ui| {
        for (i, label) in labels.iter().enumerate() {
            ui.label(*label);
            ui.add(egui::TextEdit::singleline(&mut state.args[i]).desired_width(64.0));
        }
    });
    let composed = crate::edit::compose_modifier(&state.keyword, &state.args);
    if let Err(e) = &composed {
        ui.colored_label(palette.system, e);
    }
    composed
}

fn modifier_subcontrol(ui: &mut egui::Ui, state: &mut PickerState, palette: &Palette) {
    ui.label(egui::RichText::new("MODIFIER").small().color(palette.ink_3));
    let _ = modifier_fields(ui, state, palette);
    ui.separator();
}

fn modifier_body(
    ui: &mut egui::Ui,
    state: &mut PickerState,
    palette: &Palette,
    has_modifier: &dyn Fn(&str) -> bool,
) -> PickerOutcome {
    let PickerTarget::EditModifier {
        input,
        output,
        modifier: original,
    } = state.target.clone()
    else {
        unreachable!("routed by target");
    };
    ui.heading(format!("Change modifier for {input} -> {output}"));

    // Guard: if the modifier came from a hand-edited CSV with an unknown
    // keyword or more tokens than the catalog schema expects, editing would
    // silently truncate those extra tokens on Apply. Refuse editing and show
    // the original read-only so the user must choose Cancel; the output and
    // clear affordances in the roster remain available for such rows.
    if is_unrecognized_modifier(&original) {
        ui.add_space(4.0);
        ui.colored_label(
            palette.system,
            "Unrecognized modifier — edit refused to avoid data loss.",
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(&original)
                .monospace()
                .color(palette.ink_2),
        );
        ui.add_space(6.0);
        if ui.button("Cancel").clicked() {
            return PickerOutcome::Close;
        }
        return PickerOutcome::Open;
    }

    let composed = modifier_fields(ui, state, palette);
    let commit = match &composed {
        // Engine's modifier path does not guard (input, new_modifier)
        // uniqueness; a duplicate here would silently double-key the input.
        Ok(m) if *m != original && has_modifier(m) => {
            ui.colored_label(palette.system, format!("{m} already keys another row"));
            false
        }
        Ok(_) => true,
        Err(_) => false,
    };
    // A refused engine commit re-stores the picker with the error; without
    // this label the refusal would only flash as a corner toast.
    if let Some(err) = &state.capture_error {
        ui.colored_label(palette.system, err);
    }
    ui.add_space(6.0);
    let mut outcome = PickerOutcome::Open;
    ui.horizontal(|ui| {
        if ui.add_enabled(commit, egui::Button::new("Apply")).clicked() {
            outcome = PickerOutcome::CommitModifier(composed.expect("gated on Ok"));
        }
        if ui.button("Cancel").clicked() {
            outcome = PickerOutcome::Close;
        }
    });
    outcome
}

/// Returns `true` when a modifier string cannot round-trip the catalog editor.
///
/// Unknown keyword, or more argument tokens than the schema allows (extra
/// tokens would be silently truncated on Apply). Used by `modifier_body` to
/// refuse editing rather than cause data loss.
#[must_use]
pub fn is_unrecognized_modifier(modifier: &str) -> bool {
    let mut tokens = modifier.split_whitespace();
    let keyword = tokens.next().unwrap_or("");
    if !yoke_config::catalog::Modifier::KEYWORDS.contains(&keyword) {
        return true;
    }
    let expected_args = crate::edit::modifier_arg_labels(keyword).len();
    let actual_args = tokens.count();
    actual_args > expected_args
}

/// Map an egui `Key` to the `QuadStick` catalog output id.
///
/// Only keys whose ids exist in the catalog are mapped; the
/// `every_mapped_key_is_a_known_output` test enforces this.
/// Modifier keys (Shift, Ctrl, Alt, GUI) are not reachable via egui `Key` —
/// they live in `Modifiers` — so they remain list-only.
#[must_use]
pub const fn key_to_output_id(key: egui::Key) -> Option<&'static str> {
    use egui::Key;
    Some(match key {
        Key::A => "kb_a",
        Key::B => "kb_b",
        Key::C => "kb_c",
        Key::D => "kb_d",
        Key::E => "kb_e",
        Key::F => "kb_f",
        Key::G => "kb_g",
        Key::H => "kb_h",
        Key::I => "kb_i",
        Key::J => "kb_j",
        Key::K => "kb_k",
        Key::L => "kb_l",
        Key::M => "kb_m",
        Key::N => "kb_n",
        Key::O => "kb_o",
        Key::P => "kb_p",
        Key::Q => "kb_q",
        Key::R => "kb_r",
        Key::S => "kb_s",
        Key::T => "kb_t",
        Key::U => "kb_u",
        Key::V => "kb_v",
        Key::W => "kb_w",
        Key::X => "kb_x",
        Key::Y => "kb_y",
        Key::Z => "kb_z",
        Key::Num0 => "kb_0",
        Key::Num1 => "kb_1",
        Key::Num2 => "kb_2",
        Key::Num3 => "kb_3",
        Key::Num4 => "kb_4",
        Key::Num5 => "kb_5",
        Key::Num6 => "kb_6",
        Key::Num7 => "kb_7",
        Key::Num8 => "kb_8",
        Key::Num9 => "kb_9",
        Key::F1 => "kb_f1",
        Key::F2 => "kb_f2",
        Key::F3 => "kb_f3",
        Key::F4 => "kb_f4",
        Key::F5 => "kb_f5",
        Key::F6 => "kb_f6",
        Key::F7 => "kb_f7",
        Key::F8 => "kb_f8",
        Key::F9 => "kb_f9",
        Key::F10 => "kb_f10",
        Key::F11 => "kb_f11",
        Key::F12 => "kb_f12",
        Key::Enter => "kb_enter",
        Key::Escape => "kb_escape",
        Key::Tab => "kb_tab",
        Key::Backspace => "kb_backspace",
        Key::Delete => "kb_delete",
        Key::Space => "kb_space",
        Key::ArrowUp => "kb_up_arrow",
        Key::ArrowDown => "kb_down_arrow",
        Key::ArrowLeft => "kb_left_arrow",
        Key::ArrowRight => "kb_right_arrow",
        Key::Slash => "kb_slash",
        _ => return None,
    })
}

#[cfg(test)]
const ALL_MAPPABLE: &[egui::Key] = &[
    egui::Key::A,
    egui::Key::B,
    egui::Key::C,
    egui::Key::D,
    egui::Key::E,
    egui::Key::F,
    egui::Key::G,
    egui::Key::H,
    egui::Key::I,
    egui::Key::J,
    egui::Key::K,
    egui::Key::L,
    egui::Key::M,
    egui::Key::N,
    egui::Key::O,
    egui::Key::P,
    egui::Key::Q,
    egui::Key::R,
    egui::Key::S,
    egui::Key::T,
    egui::Key::U,
    egui::Key::V,
    egui::Key::W,
    egui::Key::X,
    egui::Key::Y,
    egui::Key::Z,
    egui::Key::Num0,
    egui::Key::Num1,
    egui::Key::Num2,
    egui::Key::Num3,
    egui::Key::Num4,
    egui::Key::Num5,
    egui::Key::Num6,
    egui::Key::Num7,
    egui::Key::Num8,
    egui::Key::Num9,
    egui::Key::F1,
    egui::Key::F2,
    egui::Key::F3,
    egui::Key::F4,
    egui::Key::F5,
    egui::Key::F6,
    egui::Key::F7,
    egui::Key::F8,
    egui::Key::F9,
    egui::Key::F10,
    egui::Key::F11,
    egui::Key::F12,
    egui::Key::Enter,
    egui::Key::Escape,
    egui::Key::Tab,
    egui::Key::Backspace,
    egui::Key::Delete,
    egui::Key::Space,
    egui::Key::ArrowUp,
    egui::Key::ArrowDown,
    egui::Key::ArrowLeft,
    egui::Key::ArrowRight,
    egui::Key::Slash,
];

#[cfg(test)]
mod tests {
    use super::*;
    use yoke_config::catalog::outputs::Output;

    #[test]
    fn every_mapped_key_is_a_known_output() {
        // All keys the mapper claims to support must exist in the catalog;
        // an Unknown here would commit a binding the device cannot run.
        for key in ALL_MAPPABLE {
            let id = key_to_output_id(*key).expect("listed as mappable");
            assert!(
                !matches!(Output::from_csv(id), Output::Unknown(_)),
                "{id} not in catalog"
            );
        }
    }

    #[test]
    fn letters_digits_named_keys_map() {
        assert_eq!(key_to_output_id(egui::Key::A), Some("kb_a"));
        assert_eq!(key_to_output_id(egui::Key::Num3), Some("kb_3"));
        assert_eq!(key_to_output_id(egui::Key::F5), Some("kb_f5"));
        assert_eq!(
            key_to_output_id(egui::Key::ArrowLeft),
            Some("kb_left_arrow")
        );
        assert_eq!(key_to_output_id(egui::Key::Space), Some("kb_space"));
        // Backtick is not mapped because kb_tilde is not in the catalog.
        assert_eq!(key_to_output_id(egui::Key::Slash), Some("kb_slash"));
    }

    #[test]
    fn unmappable_keys_return_none() {
        assert_eq!(key_to_output_id(egui::Key::Copy), None);
    }

    #[test]
    fn known_modifiers_are_not_unrecognized() {
        // Every catalog modifier with its canonical arg count must pass through
        // the editor without triggering the truncation guard.
        assert!(!is_unrecognized_modifier("normal"));
        assert!(!is_unrecognized_modifier("toggle"));
        assert!(!is_unrecognized_modifier("delay_on 250"));
        assert!(!is_unrecognized_modifier("repeat 10 100"));
        assert!(!is_unrecognized_modifier("greater_than 50 80"));
    }

    #[test]
    fn unknown_keyword_is_unrecognized() {
        assert!(is_unrecognized_modifier("xyzzy"));
        assert!(is_unrecognized_modifier(""));
    }

    #[test]
    fn extra_tokens_are_unrecognized() {
        // delay_on expects 1 arg; 2 args would be silently truncated on Apply.
        assert!(is_unrecognized_modifier("delay_on 1000 extra"));
        // normal expects 0 args.
        assert!(is_unrecognized_modifier("normal spurious"));
        // repeat expects 2 args; 3 is too many.
        assert!(is_unrecognized_modifier("repeat 10 100 200"));
    }

    #[test]
    fn fewer_tokens_than_schema_are_ok() {
        // Optional trailing args: repeat with only hz filled is valid.
        assert!(!is_unrecognized_modifier("repeat 10"));
        // delay_on with no arg is valid (arg is optional).
        assert!(!is_unrecognized_modifier("delay_on"));
    }
}
