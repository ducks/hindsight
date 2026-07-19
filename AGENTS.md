# hindsight

Incident knowledge base in llm-wiki form. An agent (usually Claude Code)
maintains a markdown wiki distilled from raw incident data. The wiki, not the
raw data, is what gets read at question-time.

This repo is the tool: schema, docs, and (eventually) the code, including an
MCP server that serves a corpus to other agents. The corpus itself is a plain
directory of data that lives outside this repo and is passed to the tool.

This file is the schema: layout, conventions, and procedures. If you change a
convention, update this file in the same commit and note it in the corpus's
`wiki/log.md`.

## Corpus location

Resolution order:

1. Explicitly passed directory (`--data`, tool argument)
2. `$HINDSIGHT_DATA`
3. `$XDG_DATA_HOME/hindsight`
4. `~/.local/share/hindsight`

Agent sessions doing ingest/query/lint run from this repo (so this schema is
in context) and operate on the corpus at that location. The corpus is NOT a
git repo; `wiki/log.md` is the history mechanism. It contains internal
incident details: never copy corpus content into this repo or anywhere
public.

## Three layers

1. `<corpus>/raw/` - immutable source dumps (Discourse topics as JSON).
   Never edit these.
2. `<corpus>/wiki/` - the knowledge base. Markdown pages written and
   maintained by agents, readable by humans. This is the product.
3. `AGENTS.md` - this schema. Co-evolves with the wiki.

## Corpus layout

```
<corpus>/
├── raw/                # immutable source dumps: topic-<id>.json
└── wiki/
    ├── index.md        # catalog: every page, one line each
    ├── log.md          # append-only operations record
    ├── incidents/      # one page per incident: YYYYMMDD-<topic_id>.md
    ├── patterns/       # recurring failure modes: <slug>.md
    └── systems/        # hosts, clusters, services: <name>.md
```

## Page types

### Incident page (`incidents/YYYYMMDD-<topic_id>.md`)

The unit of ingestion. YYYYMMDD is the incident date (not the post-mortem
publication date, when they differ).

```markdown
---
topic_id: 185467
source: https://dev.discourse.org/t/185467
raw: raw/topic-185467.json
date: 2026-06-04
severity: critical | major | minor | unknown
systems: [ta03.yyz2, flex032]
patterns: [broadcom-nic-failure]
---

# <title>

One-paragraph summary: what broke, blast radius, how long, root cause.

## Timeline
## Investigation
## Root cause
## Resolution
## Lessons
```

Sections are prose, links inline. Omit a section rather than writing filler;
write `unknown` rather than guessing. Every claim should be traceable to the
raw source. Do not invent timestamps, commands, or actors.

### Pattern page (`patterns/<slug>.md`)

A recurring failure mode, distilled across incidents. Created when a second
incident matches (one occurrence is an incident, two is a pattern), or
immediately when a post-mortem explicitly names a known failure mode.

```markdown
# <human name>

What this failure is, one paragraph.

## Signature
How it presents: symptoms, error strings, log patterns (regex ok).

## Detection
Commands/checks that confirm it.

## Playbook
Immediate / Mitigation / Long-term. The traps go here too: steps people
skipped before and paid for.

## History
- [2026-06-04 ta03.yyz2](../incidents/20260604-185229.md) - one-line outcome
```

Pattern pages are the highest-value pages in the wiki. When ingesting, always
ask: does this incident update a playbook?

### System page (`systems/<name>.md`)

A host, cluster, or service that has appeared in more than one incident.
Role, known weaknesses, incident history (same link-list style as patterns).
Don't create system pages for one-off mentions; the incident page suffices.

## Conventions

- Links: relative markdown links, e.g. `[bnxt_en failures](../patterns/broadcom-nic-failure.md)`.
  Link liberally; a red link (target doesn't exist yet) is a TODO marker, not
  an error.
- Dates: ISO (YYYY-MM-DD) in prose, YYYYMMDD in filenames.
- Slugs: lowercase kebab-case.
- Frontmatter is for grepping; prose is for reading. Keep both honest.
  Frontmatter keys are load-bearing: the future MCP server filters on them.
  Don't rename or restructure them without updating this schema.
- Tone: plain, specific, past tense. Write for the on-call reading this at
  3am during the next occurrence.

## index.md

One line per page, grouped by directory:

```markdown
- [20260604-185229](incidents/20260604-185229.md) - ta03.yyz2 NIC failure, 3 clusters read-only, 18 min
```

Update it in the same operation that adds or renames a page. This is the
discovery mechanism; there is no search index. Keep the one-liners
information-dense.

## log.md

Append-only. One line per operation:

```markdown
- 2026-07-18 ingest topic-185467: +incidents/20260607-185467, ~patterns/broadcom-nic-failure, ~index
```

`+` created, `~` updated, `-` removed. Never rewrite old entries.

## Operations

### Ingest

Input: a raw topic dump in `<corpus>/raw/` (or a topic ID to fetch via the
discourse MCP tools, saving the dump first).

1. Read the raw topic in full, all posts.
2. Write or update the incident page.
3. Check existing patterns: does this match one? Update its History and,
   if the incident taught something, its Playbook. Two matching incidents
   and no pattern page yet means create one.
4. Update system pages for recurring systems.
5. Cross-link: from this incident to related incidents/patterns/systems,
   and back-link from those pages to this one.
6. Update `index.md`, append to `log.md`.

One ingest may touch many pages. That's the point: integration, not filing.

### Query

Answering "have we seen X before?" or "what do we know about Y?":

1. Read `index.md`, follow relevant links, read pages.
2. Answer with citations (links to wiki pages and source topics).
3. If synthesizing the answer produced insight not yet in the wiki (a
   connection between incidents, a pattern), write it back.

### Lint

Periodic health check. Look for:

- Red links that should now resolve, orphan pages not in `index.md`
- Contradictions between pages, stale claims ("hardware replacement
  pending" from a year ago)
- Incidents matching a pattern but missing from its History
- Systems with 2+ incidents but no system page
- `raw/` dumps with no incident page

Fix what's found, log the pass in `log.md`.
