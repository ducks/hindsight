---
name: git-ingester
description: Distill a codebase's git history into pattern and area pages in a hindsight corpus. Use when asked to index a repository, answer "how has this codebase ever done X", or write playbooks for recurring kinds of change.
---

# Ingest from git history

For a codebase whose history explains itself: prefixed commit subjects,
bodies that say why, PR numbers. Discourse core is the reference case.
The corpus procedure (which pages to write and update) is AGENTS.md's
Ingest operation; this skill covers the git-specific parts.

Status: exercised against Discourse core (last 5000 commits, blame
pass included) to write one pattern page; the full-history and
area-page paths are guidance.

## The raw layer is an index, not a dump

A history is too large to read in full, so `hindsight extract` parses
it once into `raw/git/<name>.sqlite`: commits (prefix, title, body,
PR), files touched, and the symbols each change added or removed
(Ruby defs and classes, site settings and their uses, routes,
migrations and schema ops, plugin API calls, JS classes and
functions). It is raw in the schema's sense: derived, never
hand-edited, regenerated or appended by re-running extract.

```
hindsight extract ~/src/discourse --name discourse          # full history
hindsight extract ~/src/discourse --name discourse --limit 5000
hindsight git discourse stats
```

## Asking the index

- `find` answers "which commits did X": every `--added kind[:glob]`
  and `--touches glob` must hold, so stack them to narrow.
  `find --added setting --touches 'app/controllers/*'` is "added a
  setting and changed a controller in the same commit".
  Kinds: def, class, module, setting, setting_use, route, migration,
  schema, plugin_api, js_class, js_fn.
- `show <sha>` prints the commit with its body, files, and symbols.
  Read the body; on this codebase it is the design note.
- `cochange <glob>` lists files that change alongside matching files.
  This is the map of what a change to one file usually drags along
  (its spec, its serializer, the locale file).
- `blame` is a second pass to run after extract: for each FIX,
  BUGFIX, and SECURITY commit it blames the removed lines on the
  commits that wrote them (`git blame -w` on the parent, restricted
  to the removed ranges). Incremental, newest fixes first, about half
  a second per file per fix; locales and lock files are skipped.
- `fixes [--touches glob] [--window days]` then lists correction
  pairs: each fix with the commits that wrote the lines it removed,
  weighted by lines. An origin marked "not indexed" is older than
  the extracted range; extract further back to name it.
- `undone <sha>` is the other direction: the later fixes that removed
  lines a commit wrote, which is what a commit got wrong. `show` also
  prints both directions for one commit.

Then read the actual diff with `git show` for the hunks a page needs
to quote. The index locates; git explains.

## Page mapping

- **Pattern page**: a recurring kind of change (a setting gating a
  route, a new admin route, a migration that backfills). Signature is
  what the diff looks like, Detection is the `find` query that lists
  the instances, Playbook is the variants and their traps, History
  cites the PRs. Two commits that solve the same problem the same way
  are a pattern.
- **Area page** (in `systems/`): a subsystem with a history worth
  knowing (the guardian, TopicQuery, the stylesheet pipeline). Role,
  known weaknesses from `fixes --touches` on its files, what a change
  to it drags along from `cochange`, and a dated history of the
  changes that shaped it.
- No page per commit. A commit is a citation, not a unit of ingestion.

## Frontmatter and citations

Pattern and area pages carry no source frontmatter; they cite PRs
inline as `[#NNNN](https://github.com/<owner>/<repo>/pull/NNNN)` and
dated History lines, so the wiki reads without the index. Name the
index in the Detection section so an agent can rerun the query.

## Quirks

- `--first-parent` is used, so squash-merged PRs are one commit each
  and merge commits of long-lived branches hide their inner history.
- Pre-2015 Discourse commits rarely have a prefix or a body; filter on
  `--prefix` only for recent history.
- Lock files, locales, fixtures, and vendored code are indexed as
  files but contribute no symbols.
- Symbol extraction is line-level regex, so a def moved between files
  looks like a delete and an add; confirm with `git show`.
