---
name: github-ingester
description: Ingest incidents tracked as GitHub issues into the hindsight corpus. Use when asked to ingest a GitHub issue (by URL or number), or to sweep a repo's incident/postmortem-labeled issues.
---

# Ingest from GitHub issues

For teams that run incidents through GitHub issues (commonly labeled
`incident`, `postmortem`, or `outage`). The corpus procedure (which
pages to write and update) is AGENTS.md's Ingest operation; this skill
covers the GitHub-specific parts.

Status: mapping guidance; not yet exercised against a real corpus.

## Fetching

Use the `gh` CLI (already authenticated wherever `gh` works):

1. Finding candidates:
   `gh issue list -R <owner>/<repo> --label incident --state all --json number,title,createdAt`
   (repeat per label the team uses; `--state all` matters, resolved
   incidents are closed issues).
2. Fetch the FULL issue, comments included:
   `gh issue view <number> -R <owner>/<repo> --json number,title,url,createdAt,closedAt,body,labels,comments`
   The discussion thread is where the investigation lives; the opening
   body is often just the alert.

## Raw dump

Save the JSON from `gh issue view` to
`<corpus>/raw/github-<owner>-<repo>-<number>.json`, unmodified. Never
edit a dump after saving; re-fetch to a fresh save if the issue changed
materially, and note it in `wiki/log.md`.

## Frontmatter mapping

- `source_id`: `<owner>-<repo>-<number>` (issue numbers alone collide
  across repos; the incident page filename becomes
  `YYYYMMDD-<owner>-<repo>-<number>.md`)
- `source`: the issue URL
- `raw`: `raw/github-<owner>-<repo>-<number>.json`
- `date`: the INCIDENT date from the content; `createdAt` is when the
  issue was opened, which for retroactive post-mortems is days later

## Quirks

- Issue bodies get edited in place with no visible history in the JSON;
  the dump reflects fetch time, not incident time.
- Closed does not mean resolved-as-described; read the last comments
  for the actual outcome.
- The fix is often in a linked PR, not the issue; follow cross-references
  before writing the Resolution section.
- Bot comments (CI, stale-bots, deploy notifications) can be timeline
  evidence or pure noise; use judgment, cite the useful ones.
- Reactions and +1 comments carry no incident signal; skip them.
