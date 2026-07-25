# Releasing Yoke

Yoke ships a **signed and notarized universal macOS `.dmg`** and an **unsigned
64-bit Windows NSIS installer**. Pushing a `vX.Y.Z` tag runs
[`.github/workflows/release.yml`](../.github/workflows/release.yml):

- **macOS** — builds `yoke-gui` for Apple Silicon and Intel, fuses them into a
  universal binary with `lipo`, then [`cargo-packager`][packager] bundles a
  `Yoke.app`, signs it, notarizes it with Apple, and staples the ticket.
- **Windows** — builds `x86_64-pc-windows-msvc` on a `windows-latest` runner and
  packages it as an NSIS `-setup.exe`.

Each job uploads its assets as a workflow artifact; a final `publish` job
collects them into one GitHub Release, so the two platforms cannot race to
create it.

All Apple-account specifics live in **repository secrets** — nothing is
committed. A plain `cargo packager` (or a tag push before the secrets exist)
produces an *unsigned* bundle.

There is no Windows code-signing certificate, so SmartScreen warns on first run
of the installer. Windows on ARM is not built: GitHub's `windows-11-arm` runners
are not free for public repositories.

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
| `APPLE_TEAM_ID` | your 10-character Team ID (optional — see below) |

`APPLE_TEAM_ID` is not consumed by the App Store Connect **API-key** notarization
path this pipeline uses (`notarytool` derives the team from the issuer);
`cargo-packager` reads it only for the alternative Apple-ID + app-password auth.
It is kept for that fallback and for the signing-identity suffix.

The signing identity is **not secret** (it is embedded in every signed binary),
but it is stored as a secret so no Apple-account specifics live in tracked files.

## Cutting a release

The tag **is** the version — the workflow stamps `vX.Y.Z` into the bundle and
into `yokectl --version`, so there is no `Cargo.toml` bump to remember. Use a
clean SemVer tag (`v0.1.0`); a pre-release suffix is not a valid Apple bundle
version.

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release uploads four assets:

| Asset | Contents |
|---|---|
| `Yoke_<version>_universal.dmg` | signed, notarized macOS app |
| `yokectl-universal-apple-darwin.tar.gz` | signed, notarized macOS CLI |
| `yoke-gui_<version>_x64-setup.exe` | Windows NSIS installer |
| `yokectl-x86_64-pc-windows-msvc.zip` | Windows CLI |

The workflow can also be run by hand from **Actions → release →
Run workflow** (`workflow_dispatch`); it builds with the `Cargo.toml` version,
skips publishing, and leaves the artifacts on the run for a smoke test. Without
the secrets above, the macOS artifacts are unsigned.

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
