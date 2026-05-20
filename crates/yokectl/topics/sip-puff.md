# Sip and puff

The QuadStick's primary inputs come from the sip and puff pressure sensors at
the mouthpiece. The firmware classifies each pressure event into a phrase
along two axes:

- Direction: sip (negative pressure) or puff (positive pressure).
- Magnitude / duration: soft, hard, long, very long, and the modal variants
  (`sip_only`, `puff_only`, lip-shift combinations).

`yokectl catalog inputs` lists every phrase the catalog knows about. The
phrases themselves are not configurable — they are what the firmware
classifier emits — but the thresholds that decide which phrase fires for a
given pressure trace are.

Threshold preferences (set via `yokectl set-preference` or `set-override`):

- `sip_threshold`, `puff_threshold` — the pressure level at which the firmware
  starts paying attention. Below this is noise.
- `sip_hard_threshold`, `puff_hard_threshold` — the level that promotes a soft
  sip/puff to a hard one.
- `sip_long_ms`, `puff_long_ms` — the hold duration that promotes a hard
  sip/puff to a long one.
- `deadband` — symmetric dead zone around zero pressure; suppresses spurious
  triggers when the operator is not actively sipping or puffing.

The defaults ship calibrated against the average QuadStick mouthpiece. Adjust
on a per-operator basis: `set-preference` writes the top-level value (applies
to every sub-profile); `set-override` writes a per-sub-profile override.

A typical workflow:

    yokectl set-preference my-profile.csv sip_threshold 35
    yokectl set-override my-profile.csv FPS-Mode puff_long_ms 800

Inspect the resulting profile with `yokectl show <target>`.

See also: `yokectl topic preferences`, `yokectl manual preferences`.
