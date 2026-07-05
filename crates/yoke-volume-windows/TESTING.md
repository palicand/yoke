# Testing yoke-volume-windows

Three layers, cheapest first. Automated tests run on the `windows` CI job on
every push; the layers below are the manual ones.

## Why a USB stick stands in for the QuadStick

The developer's QuadStick is also their input device: capturing it into a VM
removes host control, and QEMU/UTM passthrough does not handle the composite
MSC+HID device anyway. A plain mass-storage stick passes through fine, and
`YOKE_TEST_VIDPIDS` makes the backend treat it as a QuadStick. Only persona
re-enumeration timing needs the real device (see the last section).

## VM setup (UTM, Windows 11 ARM)

1. Install rustup in the guest, then from the repo checkout:
   `rustup show` (installs the pinned toolchain from rust-toolchain.toml).
2. Pass the USB stick to the guest via the UTM USB menu.
3. Find the stick's VID:PID: PowerShell
   `Get-PnpDevice -Class USB | Format-List InstanceId`
   and read the `VID_xxxx&PID_xxxx` pair from its entry.
4. Format the stick FAT32 and copy a few QuadStick CSVs onto it
   (label it e.g. QUADSTICK so the GUI shows a realistic label).

## Manual test matrix (stick + override)

In PowerShell, from the repo root, with the stick attached:

    $env:YOKE_TEST_VIDPIDS = "vvvv:pppp"   # the stick's IDs
    $env:YOKE_REAL_DEVICE = "1"
    cargo test -p yoke-volume-windows --test real_device -- --nocapture
    cargo run -p yokectl -- device         # expect Present + drive letter
    cargo run -p yokectl -- list
    cargo run -p yokectl -- watch          # then detach/reattach the stick:
                                           # expect VolumeUnmounted/DeviceDisappeared
                                           # and DeviceAppeared/VolumeMounted
    cargo run -p yoke-gui                  # library shows the stick's CSVs;
                                           # edit + save round-trips

Persona stickiness (two sticks, different VID:PIDs, same physical port):
put only stick A's IDs in YOKE_TEST_VIDPIDS; attach A (Present), detach,
attach stick B in the same port. Expect a DeviceVisibleNoVolume state with
the Emulation mode hint (stick B's VID:PID). This approximates the
persona flip; the exact QuadStick behavior (the same device re-enumerating
in place) is what the real-device smoke test below is for.

## Real-device smoke test (VMware Fusion)

Only for persona re-enumeration timing. Enable USB 2.0 in the VM settings
before first guest boot (QuadStick guidance). While the device is captured
by the guest the host loses it as an input device; the escape hatch is a
device-side mode change (long hard sip -> file selection -> pick a profile),
which re-enumerates the QuadStick and releases the capture.

If Fusion passthrough fails like UTM's did, record the gap in the PR and the
release notes: Windows support ships beta-caveated until a Windows-native
tester confirms persona transitions.
