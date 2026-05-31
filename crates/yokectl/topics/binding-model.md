# Binding model

A binding maps an input phrase the QuadStick recognizes to an output the host
machine receives, under an optional *modifier* that shapes the timing or
behavior of that mapping. Inputs are categorical: sip and puff variants,
lip-position states, joystick directions, and modal triggers. Outputs are also
categorical: keyboard keys, mouse motion, gamepad button presses, sub-profile
switches.

Every binding lives inside a sub-profile, not at the top level of a profile.
This is what lets the same input do different things in different modes: a sip
that types `q` while you are in keyboard mode can fire a controller button
while you are in gamepad mode.

A single input can drive **several** outputs at once — a chord — as long as
each sits under a distinct modifier. The pair `(input, modifier)` is what
uniquely identifies a binding row; the input alone does not. A binding with no
modifier defaults to `normal`. A modifier phrase is a keyword optionally
followed by arguments, e.g. `toggle` or `delay_on 250`; `yokectl catalog
modifiers` lists the keywords.

To create a binding, name the sub-profile, the input, and the output, with an
optional modifier:

    yokectl add-binding <target> <sub-profile> <input> <output> [--modifier <phrase>]

`add-binding` refuses to overwrite: if `(input, modifier)` already maps to an
output it fails rather than silently replacing it. To change an existing
binding, use `update-binding`. It takes the input, an output, and a modifier,
finds the one existing row that matches the input plus either the output or the
modifier, and sets the field you changed — so supplying the current modifier
with a new output rewrites the output, while supplying the current output with
a new modifier rewrites the modifier:

    yokectl update-binding <target> <sub-profile> <input> <output> --modifier <phrase>

The catalog is the source of truth for what names are valid. `yokectl catalog
inputs`, `yokectl catalog outputs`, and `yokectl catalog modifiers` list them.
If you spell any of them wrong the command exits with a `did you mean: [...]`
suggestion list backed by Levenshtein distance — the same machinery `yoke-edit`
uses for typo correction across the board.

To remove a binding, name the sub-profile and the input. Without a modifier
this clears **every** row for that input; pass `--modifier` to remove only the
single row carrying that modifier:

    yokectl clear-binding <target> <sub-profile> <input> [--modifier <phrase>]

For batch edits — for instance applying a whole gameplay scheme at once — use
`apply --edits <file.json>` with `add-binding` / `update-binding` /
`clear-binding` ops. Apply is all-or-nothing: if any op fails validation the
profile on disk is untouched.

See also: the upstream manual section on configuration semantics —
`yokectl manual configuration`.
