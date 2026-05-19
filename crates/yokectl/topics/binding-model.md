# Binding model

A binding maps an input phrase the QuadStick recognizes to an output the host
machine receives. Inputs are categorical: sip and puff variants, lip-position
states, joystick directions, and modal triggers. Outputs are also categorical:
keyboard keys, mouse motion, gamepad button presses, sub-profile switches.

Every binding lives inside a sub-profile, not at the top level of a profile.
This is what lets the same input do different things in different modes: a sip
that types `q` while you are in keyboard mode can fire a controller button
while you are in gamepad mode. To attach a binding you name both the
sub-profile and the input:

    yokectl set-binding <target> <sub-profile> <input> <output>

The catalog is the source of truth for what names are valid. `yokectl catalog
inputs` and `yokectl catalog outputs` list them. If you spell either side
wrong the command exits with a `did you mean: [...]` suggestion list backed by
Levenshtein distance — the same machinery `yoke-edit` uses for typo
correction across the board.

To remove a binding, name only the sub-profile and the input. The output is
implicit because each `(sub-profile, input)` pair can hold at most one
binding:

    yokectl clear-binding <target> <sub-profile> <input>

For batch edits — for instance applying a whole gameplay scheme at once — use
`apply --edits <file.json>` with `SetBinding`/`ClearBinding` ops. Apply is
all-or-nothing: if any op fails validation the profile on disk is untouched.

See also: the upstream manual section on configuration semantics —
`yokectl manual configuration`.
