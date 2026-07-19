---
name: pagerduty-ingester
description: Ingest PagerDuty incidents into the hindsight corpus. Use when asked to ingest a PagerDuty incident (by id or URL) or sweep recent PagerDuty incidents.
---

# Ingest from PagerDuty

For teams whose incident record of authority is PagerDuty. The corpus
procedure (which pages to write and update) is AGENTS.md's Ingest
operation; this skill covers the PagerDuty-specific parts.

Status: mapping guidance; not yet exercised against a real corpus.
Written from the public REST API v2 data model; verify field names
against a real response before relying on them.

## Fetching

Whatever access is configured works: a PagerDuty MCP server if one is
connected, or the REST API v2 directly with an API token
(`Authorization: Token token=<key>` against `api.pagerduty.com`). The
data needed per incident:

1. The incident itself: `GET /incidents/{id}` (title, status, urgency,
   priority, service, created_at, html_url)
2. Its timeline: `GET /incidents/{id}/log_entries` (trigger,
   acknowledge, escalate, resolve entries with timestamps and agents)
3. Human narrative: `GET /incidents/{id}/notes` (responder notes are
   often the only prose)

## Raw dump

Combine the three responses into one JSON object and save to
`<corpus>/raw/pagerduty-<id>.json` (PagerDuty ids look like `Q2X...` /
`P4B...`). Never edit a dump after saving.

## Frontmatter mapping

- `source_id`: the PagerDuty incident id
- `source`: the incident's `html_url`
- `raw`: `raw/pagerduty-<id>.json`
- `date`: from `created_at` (PagerDuty incidents are created at
  detection time, so unlike post-mortem sources this is usually the
  real incident date)
- `severity`: map from priority/urgency honestly (P1/high-urgency is
  not automatically `critical`; consider actual impact) or `unknown`

## Quirks

- A PagerDuty incident is an alert-lifecycle record, not a post-mortem:
  rich timeline, thin narrative. Expect strong Timeline sections and
  weak Root cause/Lessons sections; write `unknown` rather than
  inventing.
- The real write-up usually lives elsewhere (a doc, an issue, a forum
  topic). When one exists, ingest BOTH sources into the same incident
  page: PagerDuty for the timeline, the write-up for the analysis, each
  cited via its own raw dump.
- Several PagerDuty incidents often represent one real incident
  (re-triggers, per-service alerts): one incident page, all ids cited.
- Auto-resolved incidents may never have been investigated at all;
  they may not be worth a page.
