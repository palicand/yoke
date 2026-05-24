# Install sources

`yokectl install <source>` is deliberately undemanding about what `<source>`
means. The auto-classifier picks one of three paths based on what the
argument looks like:

1. If the argument parses as an HTTP(S) URL, the source is treated as a URL.
   The URL is fetched. If it is a Google Sheets URL it is first rewritten to
   its CSV-export form via the table in `yoke-index::url_transform`; an
   unrecognized `docs.google.com` path (e.g. `/uc?id=...`) passes through
   unchanged.
2. Otherwise, if the argument is path-like — absolute, starts with `./` or
   `../`, or contains `/` or `\` — the source is treated as a local file
   regardless of whether it currently exists. The bytes are read directly,
   parsed (unless `--no-validate`), and written to the volume.
3. Otherwise (a bare token like `Destiny 2`), the argument is treated as a
   community-index name even if a cwd file happens to share the name. The
   index is resolved (using the cached community sheet, refreshing if stale)
   and the entry's underlying CSV URL is fetched.

This means the same command serves three different intents:

    yokectl install ./my-game.csv          # local file
    yokectl install https://docs.google... # arbitrary URL
    yokectl install "Destiny 2"            # community index entry

By default `install` parses and validates the fetched bytes before writing.
`--no-validate` bypasses this and writes the bytes verbatim; a warning lands
on stderr and the JSON envelope carries `"validated": false`. Use it only
when you trust the source and want to preserve byte-exact content (a
hand-maintained CSV with deliberately unusual formatting, for instance).

`--dry-run` resolves, fetches, and (unless `--no-validate` is set) parses the
source before deciding not to write. It mirrors `apply --dry-run`: bad bytes
fail in dry-run just as they would on a real install, and the destination
name is validated before the early return so `--dry-run --as 'bad:name'`
errors with `invalid-name`. Combine with `--json` to script-test a source
before committing to it.

The destination filename comes from `--as` if provided; otherwise it is
derived from the source (index entry name, URL basename, or local file
stem). Derived names are sanitized to a FAT-safe stem: whitespace and the
FAT-illegal set (`/ \ : < > | ? * "`) collapse to `_`, ASCII letters are
lowercased, and a URL basename of `pub`/`pubhtml`/`edit`/`export` falls back
to `profile` with a warning advising `--as`. The volume backend rejects
names with path separators or anything that would not round-trip through
FAT.

When the derived destination collides with an existing profile, `install`
errors with `cli-requires-force` (exit 2) rather than overwriting silently.
Pass `--force` to overwrite, or `--as <name>` to choose a non-colliding
destination explicitly — with `--as`, the overwrite is silent and matches
`push`/`copy`.

See also: `yokectl topic preferences`, `yokectl manual google-sheets`.
