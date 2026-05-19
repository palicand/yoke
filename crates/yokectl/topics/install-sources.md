# Install sources

`yokectl install <source>` is deliberately undemanding about what `<source>`
means. The auto-classifier picks one of three paths based on what the
argument looks like:

1. If a file with that path exists on disk, the source is treated as a local
   file. The bytes are read directly, parsed, validated, and written to the
   volume.
2. Otherwise, if the argument parses as an HTTP(S) URL, the source is treated
   as a URL. The URL is fetched. If it is a Google Sheets URL it is first
   rewritten to its CSV-export form via the table in
   `yoke-index::url_transform`.
3. Otherwise, the argument is treated as a bare community-index name. The
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

`--dry-run` resolves and parses without writing. Combine with `--json` to
script-test a source before committing to it.

The destination filename comes from `--as` if provided; otherwise it is
derived from the source (index entry name, URL basename, or local file
stem). The volume backend rejects names with path separators, dots at the
start, or anything that would not round-trip through FAT.

See also: `yokectl topic preferences`, `yokectl manual google-sheets`.
