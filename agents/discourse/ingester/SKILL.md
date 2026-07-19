---
name: discourse-ingester
description: Ingest incident topics from a Discourse instance into the hindsight corpus. Use when asked to ingest a Discourse topic (by id or URL), sweep for new incident/postmortem topics, or when an incident source is a Discourse topic.
---

# Ingest from Discourse

Fetch mechanics and field mapping for Discourse-sourced incidents. The
corpus procedure itself (which pages to write and update) is AGENTS.md's
Ingest operation; this skill only covers the Discourse-specific parts.

Status: exercised; the seed corpus (35 incidents) was built with this
mapping from real topics.

## Fetching

Use the discourse MCP tools (requires a discourse MCP server):

1. If needed, `discourse_select_site` for the target instance
   (e.g. meta.discourse.org).
2. Finding candidates: `discourse_filter_topics` with incident-related
   tags (`incident`, `alert`, `outage`, `postmortem`,
   `degraded-performance`) or ask for a specific topic id.
3. `discourse_read_topic` for the FULL topic: all posts, not just the
   first. Post-mortems often live in replies, and follow-up posts
   correct the original.

## Raw dump

Save to `<corpus>/raw/topic-<id>.json` before writing any wiki page.
Shape (established convention, keep it):

```json
{
  "topic_id": 185467,
  "title": "...",
  "url": "https://meta.discourse.org/t/<slug>/<id>",
  "created_at": "ISO8601",
  "raw_content": "all posts, concatenated"
}
```

Extra top-level fields are fine; `raw_content` must contain the full
post text. Never edit a dump after saving it; re-fetch to a fresh save
if the topic changed materially, and note it in `wiki/log.md`.

## Frontmatter mapping

- `source_id`: the topic id
- `source`: the full topic URL
- `raw`: `raw/topic-<id>.json`
- `date`: the INCIDENT date, extracted from the content. Discourse
  `created_at` is the posting date; post-mortems are often written days
  later, and some topics cover multiple incidents (pick the incident the
  page is about).

## Quirks

- One topic can document several incidents (e.g. a combined post-mortem);
  one incident page per incident, both citing the same topic/raw dump.
- Timestamps in topics mix timezones freely ([date] BBCode with
  per-line timezone attributes); normalize carefully and say which zone
  a timeline uses.
- Roles blocks (IC/Scribe/Resolvers) are worth preserving on the
  incident page.
- Mock/training incidents exist (tagged or titled as such); ingest them
  clearly marked as training, or skip if they add nothing.
