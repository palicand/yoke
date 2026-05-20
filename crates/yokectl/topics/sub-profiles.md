# Sub-profiles

A profile on the QuadStick is a container; the real configuration lives in
its sub-profiles. Each sub-profile bundles together a mode, an optional
sub-mode label, a channel, and the binding table that mode/channel
combination uses.

The catalog defines the legal values:

- `yokectl catalog modes` — every `SubProfileMode` the firmware understands.
- `yokectl catalog channels` — every channel a sub-profile can target.

Sub-profile names within a single profile are unique. To add one:

    yokectl subprofile add <target> <name> --mode <m> --channel <c> [--sub-mode <s>]

Both `mode` and `channel` are validated against the catalog at command time;
unknown values surface as `did you mean: [...]` suggestions.

Lifecycle:

- `subprofile rename <target> <from> <to>` — renames in place. Bindings keep
  their `sub_profile` reference automatically because the rename mutates the
  header, not every row that names it.
- `subprofile clone <target> <from> <to>` — duplicates every binding plus the
  mode/channel/sub-mode header under a new name. Useful for forking a working
  scheme to experiment without losing the original.
- `subprofile delete <target> <name>` — removes the sub-profile and its
  bindings. The last remaining sub-profile cannot be deleted; the parser would
  reject the resulting profile.

To switch between sub-profiles on the device, bind a `change_sub_profile`
output to a trigger input. The QuadStick's mode-switch logic lives in
firmware; `yokectl` only writes the configuration that controls it.

See also: `yokectl topic binding-model`, `yokectl manual changing-profiles`.
