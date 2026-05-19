# Preferences

Preferences are typed key/value pairs that tune the QuadStick's runtime
behaviour without changing which input maps to which output. Thresholds,
deadbands, dwell timings, modal flags — all preferences. The catalog defines
the schema:

    yokectl catalog preferences

prints each key alongside its declared value type. The three types are:

- `Number` — integer (most thresholds and durations).
- `Bool` — `true` or `false`.
- `Text(allowed)` — string from a closed set of choices.

There are two scopes a preference can live in:

- Top-level — applies to every sub-profile in the file. Set with
  `set-preference <target> <key> <value>`.
- Sub-profile override — applies only inside one named sub-profile. Set with
  `set-override <target> <sub-profile> <key> <value>`.

When the firmware reads a preference it consults the active sub-profile
first; if no override is present it falls back to the top-level value. This
is what lets a single profile carry, for instance, a tight `sip_threshold`
for FPS mode and a relaxed one for a desktop-cursor sub-profile.

To remove a setting, use the corresponding unset:

    yokectl unset-preference <target> <key>
    yokectl unset-override <target> <sub-profile> <key>

The value parser infers the type from the catalog entry: `yokectl
set-preference foo.csv enable_chord_logging true` writes a `Bool`; the same
command with `35` writes a `Number`. Mismatched values produce
`InvalidPreferenceValue { key, value, expected_type }`, mapped to exit code
5.

See also: `yokectl topic binding-model`, `yokectl manual preferences`.
