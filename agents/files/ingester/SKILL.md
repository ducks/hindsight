---
name: files-ingester
description: Ingest existing post-mortem documents (markdown, text) from a directory into the hindsight corpus. Use when the user points at a folder or repo of incident write-ups, or asks to ingest a post-mortem that already exists as a file.
---

# Ingest from files

For teams whose incident history already lives as documents: a
`postmortems/` directory in a repo, exported wiki pages, meeting notes.
No fetching, no API; the input is files on disk. The corpus procedure
(which pages to write and update) is AGENTS.md's Ingest operation; this
skill covers getting file-based post-mortems into it.

Status: mapping guidance; not yet exercised against a real corpus.

## Input

A directory (or single file) of post-mortem documents. Markdown and
plain text work best; ask before attempting other formats. Skim the set
first and confirm with the user which files are actually incident
write-ups; directories mix in templates, drafts, and unrelated notes.

## Raw dump

Copy each source document verbatim to
`<corpus>/raw/file-<slug>.<original extension>`, where `<slug>` is the
original filename slugified (lowercase kebab-case, extension dropped).
The copy IS the raw layer; never edit it. If two files slugify
identically, disambiguate with a date or parent-directory prefix and
note it in `wiki/log.md`.

## Frontmatter mapping

- `source_id`: the slug
- `source`: where the document canonically lives, as a stable reference:
  a repo URL + path, a wiki URL, or an absolute path if that's all there
  is
- `raw`: `raw/file-<slug>.<ext>`
- `date`: the INCIDENT date, extracted from the content or filename.
  File mtime is a last resort and usually wrong (files get copied);
  if the date is genuinely unrecoverable, use what evidence supports
  and say so on the page.

## Quirks

- Free-form documents vary wildly in structure; extract what exists and
  omit sections rather than padding. The honesty rules in AGENTS.md do
  the heavy lifting here.
- One document sometimes covers several incidents (quarterly reviews,
  combined post-mortems): one incident page per incident, all citing the
  same raw file.
- Documents copied between systems shed their metadata (author, date);
  prefer dates and names found in the text itself.
- Check for an existing incident page before writing: file-based ingest
  is the likeliest path to accidental duplicates, since the same
  post-mortem often exists in several exports.
