use yoke_config::ParseResult;
use yoke_config::catalog::outputs::Output;
use yoke_config::catalog::{Channel, Input, Modifier, SubProfileMode};
use yoke_config::csv::raw::RawCsv;
use yoke_config::error::WriteError;
use yoke_config::model::Profile;
use yoke_edit::{EditError, EditOp, apply};

/// Op-log editing state over one opened profile. Every mutation routes
/// through `yoke_edit::apply`; on error the session is left untouched.
#[derive(Debug, Clone)]
pub struct EditSession {
    base: Profile,
    template: RawCsv,
    ops: Vec<EditOp>,
    redo: Vec<EditOp>,
    current: Profile,
    saved: Profile,
}

impl EditSession {
    #[must_use]
    pub fn new(parsed: ParseResult) -> Self {
        Self {
            current: parsed.model.clone(),
            saved: parsed.model.clone(),
            base: parsed.model,
            template: parsed.raw,
            ops: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub const fn current(&self) -> &Profile {
        &self.current
    }

    /// Dirty is a state comparison, not an op counter: undoing back to the
    /// last-saved shape must read as clean, and undoing past a save point
    /// must read as dirty. A mis-reported clean state risks silent data loss.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.current != self.saved
    }

    #[must_use]
    pub const fn can_undo(&self) -> bool {
        !self.ops.is_empty()
    }

    #[must_use]
    pub const fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn mark_saved(&mut self, snapshot: Profile) {
        self.saved = snapshot;
    }

    fn push(&mut self, op: EditOp) -> Result<(), EditError> {
        let next = apply(self.current.clone(), std::slice::from_ref(&op)).map_err(|e| e.error)?;
        self.current = next;
        self.ops.push(op);
        self.redo.clear();
        Ok(())
    }

    /// Undo the last operation and return `true` if there was anything to undo.
    ///
    /// # Panics
    ///
    /// Panics if replaying the recorded op log against the base profile fails.
    /// This cannot happen in correct usage because each op was already applied
    /// successfully in `push`, making replay deterministic.
    pub fn undo(&mut self) -> bool {
        let Some(op) = self.ops.pop() else {
            return false;
        };
        self.redo.push(op);
        // Recorded ops applied cleanly against the deterministic states before
        // them, so replay from base cannot fail.
        self.current = apply(self.base.clone(), &self.ops).expect("recorded ops replay cleanly");
        true
    }

    /// Redo the last undone operation and return `true` if there was anything to redo.
    ///
    /// # Panics
    ///
    /// Panics if re-applying the previously undone op fails. This cannot happen
    /// in correct usage because the op was already applied successfully before
    /// it was undone.
    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo.pop() else {
            return false;
        };
        self.current = apply(self.current.clone(), std::slice::from_ref(&op))
            .expect("undone op re-applies cleanly");
        self.ops.push(op);
        true
    }

    /// Add a new binding; fails with [`EditError::BindingExists`] if the
    /// `(input, modifier)` key is already occupied.
    pub fn add_binding(
        &mut self,
        sub: usize,
        input: &str,
        output: &str,
        modifier: Option<&str>,
    ) -> Result<(), EditError> {
        self.push(EditOp::AddBinding {
            sub_profile: sub,
            input: input.to_owned(),
            output: output.to_owned(),
            modifier: modifier.map(ToOwned::to_owned),
        })
    }

    /// Update an existing binding, anchored by output (changes modifier) or by
    /// modifier (changes output). Fails if the target row is not found.
    pub fn update_binding(
        &mut self,
        sub: usize,
        input: &str,
        output: &str,
        modifier: &str,
    ) -> Result<(), EditError> {
        self.push(EditOp::UpdateBinding {
            sub_profile: sub,
            input: input.to_owned(),
            output: output.to_owned(),
            modifier: modifier.to_owned(),
        })
    }

    /// Clear one or all binding rows for an input. When `modifier` is `Some`,
    /// only the row whose modifier matches is removed.
    pub fn clear_binding(
        &mut self,
        sub: usize,
        input: &str,
        modifier: Option<&str>,
    ) -> Result<(), EditError> {
        self.push(EditOp::ClearBinding {
            sub_profile: sub,
            input: input.to_owned(),
            modifier: modifier.map(ToOwned::to_owned),
        })
    }

    /// Append a new sub-profile with no bindings.
    pub fn add_sub_profile(
        &mut self,
        name: &str,
        mode: SubProfileMode,
        sub_mode: &str,
        channel: Channel,
    ) -> Result<(), EditError> {
        self.push(EditOp::AddSubProfile {
            name: name.to_owned(),
            mode,
            sub_mode: sub_mode.to_owned(),
            channel,
        })
    }

    /// Clone an existing sub-profile under a new name.
    pub fn clone_sub_profile(&mut self, index: usize, to: &str) -> Result<(), EditError> {
        self.push(EditOp::CloneSubProfile {
            index,
            to: to.to_owned(),
        })
    }

    /// Rename a sub-profile.
    pub fn rename_sub_profile(&mut self, index: usize, to: &str) -> Result<(), EditError> {
        self.push(EditOp::RenameSubProfile {
            index,
            to: to.to_owned(),
        })
    }

    /// Delete a sub-profile; fails with [`EditError::LastSubProfileDeletion`]
    /// if only one remains.
    pub fn delete_sub_profile(&mut self, index: usize) -> Result<(), EditError> {
        self.push(EditOp::DeleteSubProfile { index })
    }

    /// Pre-check: returns `true` if `(input, modifier)` is already occupied in
    /// the given sub-profile. Use this before `add_binding` when you want to
    /// offer the user a "this slot is taken" hint before trying the op.
    #[must_use]
    pub fn has_binding(&self, sub: usize, input: &str, modifier: &str) -> bool {
        let Some(sp) = self.current.sub_profiles.get(sub) else {
            return false;
        };
        let Some(m) = Modifier::from_csv(modifier) else {
            return false;
        };
        let input = Input::from_csv(input);
        sp.bindings()
            .any(|b| b.input.as_ref() == Some(&input) && b.modifier == m)
    }

    /// Template-fidelity write; any structural sub-profile edit (add/clone/
    /// delete) in the op log forces canonical layout instead.
    ///
    /// The count-mismatch fallback alone is not enough: the template writer
    /// maps sections by index, so a count-preserving reorder (clone then
    /// delete) would pass the invariant check and silently weld the old
    /// mode/sub-mode/channel header cells onto the wrong sub-profiles.
    pub fn serialize(&self) -> Result<Vec<u8>, WriteError> {
        if self.ops.iter().any(|op| {
            matches!(
                op,
                EditOp::AddSubProfile { .. }
                    | EditOp::CloneSubProfile { .. }
                    | EditOp::DeleteSubProfile { .. }
            )
        }) {
            return yoke_config::write(&self.current, None);
        }
        match yoke_config::write(&self.current, Some(&self.template)) {
            Ok(bytes) => Ok(bytes),
            Err(WriteError::InvariantViolation(_)) => {
                tracing::warn!(
                    "template invalidated by structural edit; falling back to canonical layout"
                );
                yoke_config::write(&self.current, None)
            }
        }
    }
}

/// Category labels for the known `Output` variants (excluding `Unknown`).
pub const CATEGORIES: &[&str] = &[
    "Keyboard", "Mouse", "Gamepad", "Dpad", "Joystick", "System", "Touch",
];

/// Map an `Output` to its category label. Returns `"Other"` for `Output::Unknown`.
#[must_use]
pub const fn output_category(output: &Output) -> &'static str {
    match output {
        Output::Keyboard(_) => "Keyboard",
        Output::Mouse(_) => "Mouse",
        Output::Gamepad(_) => "Gamepad",
        Output::Dpad(_) => "Dpad",
        Output::Joystick(_) => "Joystick",
        Output::System(_) => "System",
        Output::Touch => "Touch",
        Output::Unknown(_) => "Other",
    }
}

/// Field labels for each modifier keyword's optional arguments, in positional
/// order. Mirrors `Modifier::from_csv`'s arity guards so the UI can render
/// the right number of input fields.
#[must_use]
pub fn modifier_arg_labels(keyword: &str) -> &'static [&'static str] {
    match keyword {
        "delay_on" | "delay_off" | "duty" | "force_off" | "delayed_latch" => &["ms"],
        "greater_than" => &["pct", "upper"],
        "less_than" => &["pct"],
        "repeat" => &["hz", "delay_ms"],
        "pulse" => &["ms", "count"],
        "tap" => &["window_ms", "pulse_ms"],
        "increment_value" | "decrement_value" => &["amount", "interval_ms"],
        _ => &[],
    }
}

/// Build the CSV form of a modifier from a keyword and positional argument fields.
///
/// Validates by round-trip through `Modifier::from_csv`. Arguments are optional
/// trailing values; a gap (empty field before a filled one) is refused because it
/// cannot be expressed in CSV form.
///
/// # Errors
///
/// Returns an error string if a gap is found before a non-empty argument, or
/// if the assembled CSV does not parse back to the expected modifier keyword.
pub fn compose_modifier(keyword: &str, args: &[String]) -> Result<String, String> {
    let mut parts: Vec<&str> = vec![keyword];
    let mut gap = false;
    for arg in args {
        let arg = arg.trim();
        if arg.is_empty() {
            gap = true;
        } else if gap {
            return Err("fill earlier arguments first".to_owned());
        } else {
            parts.push(arg);
        }
    }
    let csv = parts.join(" ");
    match Modifier::from_csv(&csv) {
        Some(m) if m.keyword() == Some(keyword) => Ok(csv),
        _ => Err(format!("invalid arguments for {keyword}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_SUBS: &[u8] = b"QuadStick Configuration,Version 1.4,,T\r\n\
Profile Name,,Mouse Mode,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
mouse_left,normal,left,\r\n\
kb_left_gui,normal,lip,\r\n\
kb_left_shift,delay_on 1000,lip,\r\n\
\r\n\
Profile Name,,Left joy,\r\n\
,,Normal,\r\n\
Output or Function,Function,usb,\r\n\
kb_a,normal,left,\r\n\
\r\n";

    fn session() -> EditSession {
        EditSession::new(yoke_config::parse(TWO_SUBS).expect("fixture parses"))
    }

    fn outputs_for(
        p: &yoke_config::model::Profile,
        sub: usize,
        input: &str,
    ) -> Vec<(String, String)> {
        let input = yoke_config::catalog::Input::from_csv(input);
        p.sub_profiles[sub]
            .bindings()
            .filter(|b| b.input.as_ref() == Some(&input))
            .map(|b| (b.modifier.to_csv(), b.output.to_csv()))
            .collect()
    }

    #[test]
    fn new_session_is_clean() {
        let s = session();
        assert!(!s.is_dirty());
        assert!(!s.can_undo());
        assert!(!s.can_redo());
    }

    #[test]
    fn add_binding_targets_only_its_sub_profile() {
        let mut s = session();
        s.add_binding(1, "lip", "kb_b", None).unwrap();
        assert_eq!(outputs_for(s.current(), 1, "lip").len(), 1);
        // sub 0 keeps its chord untouched (the corruption class the index slice fixed)
        assert_eq!(outputs_for(s.current(), 0, "lip").len(), 2);
        assert!(s.is_dirty());
    }

    #[test]
    fn update_output_anchored_by_modifier() {
        let mut s = session();
        s.update_binding(0, "lip", "kb_z", "normal").unwrap();
        let rows = outputs_for(s.current(), 0, "lip");
        assert!(rows.contains(&("normal".into(), "kb_z".into())));
        assert!(rows.contains(&("delay_on 1000".into(), "kb_left_shift".into())));
    }

    #[test]
    fn update_modifier_anchored_by_output() {
        let mut s = session();
        s.update_binding(0, "lip", "kb_left_gui", "toggle").unwrap();
        assert!(
            outputs_for(s.current(), 0, "lip").contains(&("toggle".into(), "kb_left_gui".into()))
        );
    }

    #[test]
    fn clear_one_chord_row_keeps_the_other() {
        let mut s = session();
        s.clear_binding(0, "lip", Some("delay_on 1000")).unwrap();
        let rows = outputs_for(s.current(), 0, "lip");
        assert_eq!(rows, vec![("normal".into(), "kb_left_gui".into())]);
    }

    #[test]
    fn clear_all_rows_for_input() {
        let mut s = session();
        s.clear_binding(0, "lip", None).unwrap();
        assert!(outputs_for(s.current(), 0, "lip").is_empty());
    }

    #[test]
    fn failed_op_leaves_state_untouched() {
        let mut s = session();
        let err = s.add_binding(0, "lip", "kb_q", None).unwrap_err();
        assert!(matches!(err, yoke_edit::EditError::BindingExists { .. }));
        assert!(!s.is_dirty());
        assert!(!s.can_undo());
    }

    #[test]
    fn undo_redo_round_trip() {
        let mut s = session();
        s.add_binding(1, "lip", "kb_b", None).unwrap();
        assert!(s.undo());
        assert!(!s.is_dirty());
        assert!(s.can_redo());
        assert!(s.redo());
        assert!(s.is_dirty());
        assert_eq!(outputs_for(s.current(), 1, "lip").len(), 1);
    }

    #[test]
    fn rename_then_edit_replays_deterministically() {
        let mut s = session();
        s.rename_sub_profile(1, "Cougar").unwrap();
        s.add_binding(1, "lip", "kb_b", None).unwrap();
        assert!(s.undo());
        assert!(s.undo());
        assert!(!s.is_dirty());
        assert!(s.redo());
        assert!(s.redo());
        assert_eq!(s.current().sub_profiles[1].header.profile_name, "Cougar");
        assert_eq!(outputs_for(s.current(), 1, "lip").len(), 1);
    }

    #[test]
    fn has_binding_pre_check() {
        let s = session();
        assert!(s.has_binding(0, "lip", "normal"));
        assert!(s.has_binding(0, "lip", "delay_on 1000"));
        assert!(!s.has_binding(0, "lip", "toggle"));
        assert!(!s.has_binding(1, "lip", "normal"));
    }

    #[test]
    fn serialize_unedited_is_byte_identical_to_input() {
        let s = session();
        assert_eq!(s.serialize().unwrap(), TWO_SUBS.to_vec());
    }

    #[test]
    fn serialize_falls_back_to_canonical_after_structural_edit() {
        let mut s = session();
        s.add_sub_profile(
            "Extra",
            yoke_config::catalog::SubProfileMode::Mouse,
            "Normal",
            yoke_config::catalog::Channel::Usb,
        )
        .unwrap();
        let bytes = s.serialize().expect("canonical fallback");
        let reparsed = yoke_config::parse(&bytes).expect("fallback output parses");
        assert_eq!(reparsed.model.sub_profiles.len(), 3);
    }

    #[test]
    fn serialize_after_count_preserving_reorder_keeps_headers_welded() {
        let mut s = session();
        // Clone sub 0 then delete it: the section count nets back to the
        // template's, but the surviving sub-profiles are reordered. A
        // template-fidelity write would map headers by index and swap modes.
        s.clone_sub_profile(0, "Copy").unwrap();
        s.delete_sub_profile(0).unwrap();
        let bytes = s.serialize().expect("canonical write");
        let reparsed = yoke_config::parse(&bytes).expect("output parses");
        assert_eq!(reparsed.model, *s.current());
    }

    #[test]
    fn delete_last_sub_profile_refused() {
        let mut s = session();
        s.delete_sub_profile(0).unwrap();
        let err = s.delete_sub_profile(0).unwrap_err();
        assert!(matches!(err, yoke_edit::EditError::LastSubProfileDeletion));
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut s = session();
        s.add_binding(1, "lip", "kb_b", None).unwrap();
        let snapshot = s.current().clone();
        s.mark_saved(snapshot);
        assert!(!s.is_dirty());
        // undo past the save point makes it dirty again (honest comparison, not a counter)
        assert!(s.undo());
        assert!(s.is_dirty());
    }

    #[test]
    fn compose_modifier_validates_inline() {
        assert_eq!(compose_modifier("normal", &[]), Ok("normal".into()));
        assert_eq!(
            compose_modifier("delay_on", &["250".into()]),
            Ok("delay_on 250".into())
        );
        // optional trailing arg may stay empty
        assert_eq!(
            compose_modifier("repeat", &["10".into(), String::new()]),
            Ok("repeat 10".into())
        );
        // gap before a filled arg is refused
        assert!(compose_modifier("repeat", &[String::new(), "5".into()]).is_err());
        // non-numeric arg is refused
        assert!(compose_modifier("delay_on", &["abc".into()]).is_err());
    }

    #[test]
    fn every_keyword_has_arg_labels_and_round_trips() {
        for kw in yoke_config::catalog::Modifier::KEYWORDS {
            let labels = modifier_arg_labels(kw);
            // bare keyword must always be a valid modifier (all args are optional)
            assert!(
                compose_modifier(kw, &vec![String::new(); labels.len()]).is_ok(),
                "{kw}"
            );
        }
    }

    #[test]
    fn output_category_covers_known_outputs() {
        for o in yoke_config::catalog::outputs::Output::iter_known() {
            assert!(CATEGORIES.contains(&output_category(&o)), "{}", o.to_csv());
        }
    }
}
