# yokectl — browse and topics addendum

- **Date:** 2026-05-19
- **Status:** Proposed, awaiting review
- **Parent spec:** [2026-05-18-yokectl-design.md](./2026-05-18-yokectl-design.md)
- **Sibling addendum:** [2026-05-19-yokectl-docs-addendum.md](./2026-05-19-yokectl-docs-addendum.md)
- **Sub-project ID:** D (`yokectl`)

## Context

`yokectl --help` (after the docs addendum) is now a complete machine reference. What it does not cover is the conceptual material an operator needs before they reach for a command: what a sub-profile is, how the binding model works, what the preference catalog represents semantically. That material exists in the upstream QuadStick user manual, but the CLI currently offers no path to it — no link, no in-tool primer, nothing.

This addendum lands two complementary surfaces:

- A small set of in-binary topic pages for the concept material that has no good home in `--help`.
- Browser-launch subcommands that hand off to the upstream sheet and manual when the operator wants the source.

## Goals

1. `yokectl topic [<name>]` shows curated conceptual documentation that ships with the binary.
2. `yokectl manual [<topic>]` opens the upstream QuadStick user manual (root or named sub-page) in the operator's default browser.
3. `yokectl index browse` opens the community-profile sheet in the browser (the published HTML form, not the CSV-export form).
4. JSON mode (`--json`) returns the URL or content as data instead of side-effecting the browser, so an agent or script can route it elsewhere.

## Non-goals

- **No mirroring of upstream documentation.** The QuadStick user manual is a stable third-party HTML site. Re-hosting it in the repo would duplicate content and create a sync burden with no upside; `manual` punts the deep content to the source.
- **No HTML rendering in the terminal.** Topic pages are plain markdown emitted to stdout. Operators who want pretty rendering can pipe through `glow`, `mdcat`, or `bat -l md`.
- **No fetched content.** Topics are baked in via `include_str!` at build time. Reduces runtime failure modes; topic edits go through review like any other code.
- **No browser detection or substitution.** Whatever the OS resolves as the default browser is what gets launched. No `$BROWSER` override beyond what `opener` already honors.

## Design

### 1. Browser-launch helper

Add `opener = "0.7"` to `yokectl`'s `[dependencies]`. The crate is ~200 lines of glue around platform-native `open` / `xdg-open` / `cmd /c start`; no native deps.

A small helper in `yokectl::commands::open_url` wraps the call so the three subcommands share error handling:

```rust
pub(crate) fn open_or_emit(url: &Url, out: &Output) -> Result<()> {
    if out.is_json() {
        out.emit_json(json!({ "url": url.as_str() }));
        return Ok(());
    }
    opener::open_browser(url.as_str())
        .map_err(|e| anyhow::anyhow!("failed to launch browser for {url}: {e}"))?;
    println!("opened {url}");
    Ok(())
}
```

`out.is_json()` is a new (cheap) accessor on `Output`; under JSON the URL is data, never a side-effect. Browser-launch failure maps to exit code 6 (I/O) — same class as a filesystem write failure, on the grounds that the failure mode is "external resource handoff broke," not a logic error.

### 2. `yokectl index browse`

New variant in `IndexCmd::Browse`. Opens the published HTML form of the community sheet.

The hardcoded `COMMUNITY_INDEX_URL` in `yoke-index` is the CSV-export form (`…/d/e/{KEY}/pub?…&output=csv`). The browser-friendly counterpart is the `pubhtml?...` form. We add a sibling constant:

```rust
pub const COMMUNITY_INDEX_HTML_URL: &str =
    "https://docs.google.com/spreadsheets/d/e/\
     2PACX-1vTdyPHsW5dHAgR8DKwQ3hB9hAF1SnrIrYsCt6qvEsPSWB7MxvIVyGFVNQCgD_RcRQRYB8_ncXCYB_EI/\
     pubhtml?gid=1483029791&single=true";
```

Hand-paired with `COMMUNITY_INDEX_URL` rather than derived, because the transform (`pub?…output=csv` → `pubhtml?…`) is conceptually reverse-direction from the existing forward transform in `url_transform.rs` and introducing a reverse path muddies the semantics for one constant.

### 3. `yokectl manual [<topic>]`

New top-level variant `Commands::Manual { topic: Option<String> }`.

```rust
const MANUAL_ROOT: &str = "https://quadstick.s3.amazonaws.com/documents/user_manual/um/configuration.htm";

const MANUAL_TOPICS: &[(&str, &str)] = &[
    ("configuration", "configuration.htm"),
    ("google-sheets", "google_drive_spreadsheets.htm"),
    ("sip-puff",      "sip_puff.htm"),
    ("joystick",      "joystick.htm"),
    ("keyboard",      "keyboard.htm"),
    ("mouse",         "mouse.htm"),
    ("modes",         "modes.htm"),
    ("preferences",   "preferences.htm"),
];
```

The exact filename list will be confirmed against the upstream site at implementation time; entries that 404 are dropped from the table before commit. `yokectl manual` with no topic opens `MANUAL_ROOT`. `yokectl manual <topic>` resolves the slug in `MANUAL_TOPICS`; unknown slugs error with exit code 2 (argument) and a `did you mean: [...]` suggestion list built via `strsim` (same pattern as `yoke-edit::EditError::UnknownInput`).

`yokectl manual --json` lists the topic table:

```json
{ "root": "…/configuration.htm",
  "topics": [
    { "slug": "sip-puff", "url": "…/sip_puff.htm" },
    …
  ] }
```

`yokectl manual <topic> --json` returns `{ "slug": "sip-puff", "url": "…/sip_puff.htm" }` without launching.

### 4. `yokectl topic [<name>]`

New top-level variant `Commands::Topic { name: Option<String> }`.

Topic content lives at `crates/yokectl/topics/*.md`. Each file is hand-authored markdown that paraphrases an upstream concept in CLI-operator-flavored language and cross-links to the upstream page via a footer line. Initial set:

| Slug | First-cut content |
|---|---|
| `binding-model` | What a binding is; the input phrase → output mapping; how `set-binding` constructs one; catalog grounding. |
| `sub-profiles` | Sub-profile / mode / sub-mode / channel concepts; how they nest; how `subprofile add` builds them. |
| `sip-puff` | Sip and puff input variants; threshold and deadband preferences; the relevant `set-preference` keys. |
| `preferences` | The preference catalog; preference types (Number/Bool/Text); how `set-preference` and `set-override` differ. |
| `install-sources` | The `install` auto-classification rules (path vs URL vs index name); Google-Sheets URL rewriting. |

Topics register via a const table in `commands/topic.rs`:

```rust
const TOPICS: &[(&str, &str)] = &[
    ("binding-model",   include_str!("../topics/binding-model.md")),
    ("sub-profiles",    include_str!("../topics/sub-profiles.md")),
    ("sip-puff",        include_str!("../topics/sip-puff.md")),
    ("preferences",     include_str!("../topics/preferences.md")),
    ("install-sources", include_str!("../topics/install-sources.md")),
];
```

UX:

- `yokectl topic` — lists slugs and their first-line title, table or JSON.
- `yokectl topic <slug>` — emits the markdown body to stdout, raw.
- `yokectl topic <slug> --json` — `{ "slug": "...", "title": "...", "body": "..." }`.
- Unknown slug → exit code 2 with `strsim` suggestions.

### 5. Output discipline

All three new commands honor the existing JSON-vs-human discipline (parent § 7):

- Human stdout: a one-line confirmation for `manual` and `index browse`; the markdown body for `topic <slug>`; an aligned table for `topic` list and `manual --json`-like listing.
- JSON stdout: `{ "url": ... }` (manual / index browse), `{ "slug", "title", "body" }` (topic show), `{ "root", "topics": [...] }` (manual list), `{ "topics": [...] }` (topic list).
- stderr: tracing only.

### 6. Error handling

| Failure | Exit code |
|---|---|
| Unknown topic slug (topic / manual) | 2 |
| Browser launch failed | 6 |
| Other I/O on JSON emit | 6 |

No new exit codes added.

## Tests

- **Topic content presence.** Compile-time check via `include_str!`; runtime test asserts every entry in `TOPICS` decodes as UTF-8 and starts with `# `.
- **Topic listing.** `yokectl topic` lists every slug in `TOPICS`.
- **Topic show.** `yokectl topic binding-model` stdout starts with `# `.
- **Topic suggestions.** `yokectl topic bindng-model` (typo) exits 2 with a stderr message containing `did you mean: ["binding-model"]`.
- **Manual listing.** `yokectl manual --json` parses as JSON with `root` and a non-empty `topics` array.
- **Manual JSON for a known topic.** `yokectl manual sip-puff --json` returns the resolved URL without launching.
- **Index browse JSON.** `yokectl index browse --json` returns the HTML URL without launching.
- **No browser-launch tests.** Integration tests do not exercise the `opener` path because CI runners have no usable browser; the JSON mode is the testable surface.

## CI

No new CI step required. The existing `cargo test --workspace` covers the new tests. The real-network smoke landed in `ci(yoke-index): run real-network community-index smoke` already exercises the cached index path that `index browse` shares.

## Acceptance criteria

1. `yokectl topic`, `yokectl topic <slug>`, `yokectl topic <slug> --json` all work; the five topic pages listed in § 4 exist with non-trivial content (≥ 20 lines each).
2. `yokectl manual`, `yokectl manual <topic>`, `yokectl manual --json` all work; the topic table has at least the five entries listed in § 3 and every URL returns 2xx on a manual check at implementation time.
3. `yokectl index browse` and `yokectl index browse --json` both work.
4. `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo test --workspace` all clean on macOS + Linux.
5. The parent spec § 3.7 (`install`) gains a one-line cross-reference to `topic install-sources`; § 3.8 (community index) gains a one-line cross-reference to `index browse`; no other parent-spec edits.

## Out of scope

- HTML rendering of topics in the terminal. Operators pipe markdown through their preferred renderer.
- Fetching topic content from a URL at runtime. Topics ship in the binary; updates land via PR.
- Search across topics. Five small pages do not need fuzzy search; `grep`-friendly markdown is enough.
- Multi-language topic content. Single language for now; localization is a separate, larger conversation.
- Manual sub-page mirroring. Out of scope as a principle, not just for the current pass.

## Forward references

- The `topic` registry pattern (slug → `include_str!` body) is a candidate template for future curated-content surfaces (e.g. a `recipe` command for common edit sequences, if ever needed).
- If topic content drifts from upstream, we add a CI step that hits each `MANUAL_TOPICS` URL with a HEAD request and fails on non-2xx, treating URL rot the same way the existing community-index smoke treats sheet drift.
