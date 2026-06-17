# Releasing Yoke

Yoke ships a **signed and notarized universal macOS `.dmg`**. Pushing a
`vX.Y.Z` tag runs [`.github/workflows/release.yml`](../.github/workflows/release.yml):
it builds `yoke-gui` for both Apple Silicon and Intel, fuses them into a
universal binary with `lipo`, then [`cargo-packager`][packager] bundles a
`Yoke.app`, signs it, notarizes it with Apple, staples the ticket, and uploads
the DMG to a GitHub Release.

All Apple-account specifics live in **repository secrets** — nothing is
committed. A plain `cargo packager` (or a tag push before the secrets exist)
produces an *unsigned* bundle.

## One-time setup

You need a paid [Apple Developer Program][adp] membership.

### 1. Developer ID Application certificate

This is the certificate type for distributing apps **outside** the Mac App
Store — not "Apple Distribution" or "Mac App Store".

1. In Xcode (Settings → Accounts → Manage Certificates → `+`) or the
   [Certificates portal][certs], create a **Developer ID Application**
   certificate.
2. Export it from **Keychain Access** as a `.p12` (select the cert *and* its
   private key → right-click → Export). Set an export password.
3. Note the full identity string — `Developer ID Application: Your Name (TEAMID)`.
   Find it with: `security find-identity -v -p codesigning`.
4. Base64-encode the `.p12` for the secret:
   `base64 -i Certificates.p12 | pbcopy`

### 2. App Store Connect API key (for notarization)

1. In [App Store Connect → Users and Access → Integrations → App Store Connect
   API][asc-api], create a **Team key** with the **Developer** role (App Manager
   also works).
2. Download the `.p8` **once** (it cannot be re-downloaded). Note the **Key ID**
   and the team's **Issuer ID** shown on that page.
3. Base64-encode the `.p8` for the secret: `base64 -i AuthKey_XXXX.p8 | pbcopy`

### 3. Team ID

Your 10-character Team ID, shown in the [Membership][membership] page or as the
parenthesized suffix of the signing identity.

## Repository secrets

Add these under **Settings → Secrets and variables → Actions → New repository
secret**:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the `Developer ID Application` `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_API_KEY` | App Store Connect API **Key ID** |
| `APPLE_API_ISSUER` | App Store Connect API **Issuer ID** |
| `APPLE_API_KEY_CONTENT` | base64 of the `.p8` key file |
| `APPLE_TEAM_ID` | your 10-character Team ID |

The signing identity is **not secret** (it is embedded in every signed binary),
but it is stored as a secret so no Apple-account specifics live in tracked files.

## Cutting a release

```sh
# bump the version in crates/yoke-gui/Cargo.toml, commit it, then:
git tag v0.1.0
git push origin v0.1.0
```

The workflow can also be run by hand from **Actions → release →
Run workflow** (`workflow_dispatch`); without the secrets above it produces an
unsigned DMG.

## Verifying a release

Download the DMG from the Release, then:

```sh
# Gatekeeper assessment must pass on a machine that never saw the cert:
spctl -a -vvv --type install /Volumes/Yoke/Yoke.app
# The notarization ticket must be stapled:
xcrun stapler validate /Volumes/Yoke/Yoke.app
```

## Building an unsigned bundle locally

No certificate required — useful for smoke-testing the bundle:

```sh
cargo build --release -p yoke-gui --target aarch64-apple-darwin
cargo build --release -p yoke-gui --target x86_64-apple-darwin
mkdir -p target/universal-apple-darwin/release
lipo -create -output target/universal-apple-darwin/release/yoke-gui \
  target/aarch64-apple-darwin/release/yoke-gui \
  target/x86_64-apple-darwin/release/yoke-gui
cargo packager --release --target universal-apple-darwin -p yoke-gui
```

**Nix devShell note.** Inside this repo's `nix develop` shell, `DEVELOPER_DIR`
points at the Nix apple-sdk, which lacks `SetFile` — so the DMG's volume-icon
step fails with `tool 'SetFile' not found`. Either run `cargo packager` outside
the devShell, or point it at full Xcode for that command:

```sh
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  cargo packager --release -p yoke-gui --formats dmg
```

CI is unaffected — the release workflow does not use Nix, so its `xcode-select`
already resolves to Xcode.

## Recommended repository settings

Not required for releases, but expected for a public project:

- Set the repo **description** and **homepage**.
- Protect `main` (require PRs / status checks).

[packager]: https://github.com/crabnebula-dev/cargo-packager
[adp]: https://developer.apple.com/programs/
[certs]: https://developer.apple.com/account/resources/certificates/list
[asc-api]: https://appstoreconnect.apple.com/access/integrations/api
[membership]: https://developer.apple.com/account#MembershipDetailsCard
