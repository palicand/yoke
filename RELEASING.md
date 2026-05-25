# Releasing

Tag a commit with `v*` (e.g. `v0.1.0`) to trigger the macOS release workflow at
`.github/workflows/release.yml`. The workflow builds, signs, and notarizes a
DMG, then attaches it to the GitHub Release.

## Required GitHub Secrets

- `APPLE_CERTIFICATE` — base64-encoded `.p12` of the Developer ID Application certificate
- `APPLE_CERTIFICATE_PASSWORD` — password for the `.p12`
- `KEYCHAIN_PASSWORD` — any string, used to gate the CI temp keychain
- `APPLE_SIGNING_IDENTITY` — e.g. `Developer ID Application: Name (TEAMID)`
- `APPLE_API_ISSUER` — App Store Connect API key issuer UUID
- `APPLE_API_KEY_ID` — App Store Connect API key ID (10-char string)
- `APPLE_API_KEY` — contents of the `.p8` API key file

The certificate and API-key secrets feed `tauri-build`'s notarization step;
without them, the build still produces a `.dmg`, but unsigned bundles trigger
Gatekeeper warnings on user machines.
