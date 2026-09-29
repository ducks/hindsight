# hindsight

Incident knowledge base in [llm-wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)
form: an agent-maintained markdown wiki distilled from real production
incidents, so that the next investigation starts with everything the last
one learned.

Powers **Captain Hindsight**, the automated investigation agent who always
knows what went wrong... after the fact.

Instead of retrieving raw post-mortems at question-time (RAG), an agent
incrementally builds and maintains a wiki: incident pages, recurring failure
patterns with playbooks, per-system histories, all cross-linked. Ingesting one
incident may update many pages. The synthesis happens at ingest time; at
question-time, agents and humans just read markdown.

## Architecture

Three pieces, deliberately separate:

1. **The corpus** - a plain directory of data. `raw/` holds immutable source
   dumps; `wiki/` holds the knowledge base. Not a git repo, not part of this
   repo, and typically private: it contains real incident details. The wiki's
   `log.md` is an append-only record of every operation performed on it.
2. **The tool** (this repo) - the schema ([AGENTS.md](AGENTS.md)) defining
   wiki conventions and the ingest/query/lint procedures, and eventually the
   code: an `init` command to scaffold a corpus, and an MCP server that
   serves a corpus to any agent.
3. **The agents** - whatever reads and writes the corpus. Today: Claude Code
   sessions run from this repo, following the schema. Eventually: Captain
   Hindsight, reacting to alerts by querying the corpus over MCP.

The corpus location is resolved as: explicitly passed directory, then
`$HINDSIGHT_DATA`, then `$XDG_DATA_HOME/hindsight`, then
`~/.local/share/hindsight`.

## Corpus layout

```
<corpus>/
├── raw/                # immutable source dumps (e.g. topic-<id>.json)
└── wiki/
    ├── index.md        # catalog: every page, one line each
    ├── log.md          # append-only operations record
    ├── incidents/      # one page per incident: YYYYMMDD-<id>.md
    ├── patterns/       # recurring failure modes with playbooks
    └── systems/        # hosts, clusters, services with incident histories
```

## Operations

- **Ingest**: read a raw incident in full, write/update the incident page,
  update matching pattern and system pages, cross-link, update index and log.
  Integration, not filing.
- **Query**: "have we seen X before?" Read the index, follow links, answer
  with citations. Insight produced while answering gets written back.
- **Lint**: periodic health check for red links, orphans, contradictions,
  stale claims, incidents missing from pattern histories, raw dumps never
  ingested.

## What this enables

When a new alert fires, an agent (or a human at 3am) can learn in seconds:

- this matches a known pattern, here is the check that confirms it
- here is the playbook, including the step people skipped last time and
  paid for
- this host has failed this way three times; here is its history

And because pattern pages are distilled playbooks, they double as scenario
feedstock for [replaybook](https://github.com/ducks/replaybook), turning real
incidents into training material.

## Usage

Build (Rust; on NixOS just `nix-shell` in the repo):

```
cargo build --release
```

Commands (all respect `--data <dir>` and the resolution order above):

```
hindsight init    # scaffold a corpus directory
hindsight serve   # MCP server (stdio) over the corpus
hindsight lint    # broken links + index coverage check
hindsight path    # print the resolved corpus path

hindsight extract <repo> [--name n] [--limit n]     # index a git history
hindsight git <name> stats|show|find|cochange       # query it
hindsight git <name> blame                          # who wrote what each fix removed
hindsight git <name> fixes|undone                   # correction pairs from that
```

`extract` is for a source too big to read: it parses a repository's
history into `raw/git/<name>.sqlite` (commits, files, and the symbols
each change added or removed), and the `git` subcommands answer "which
commits ever did X" at ingest time. Agents then write pattern and area
pages from the answers, following
[agents/git/ingester](agents/git/ingester/SKILL.md). Discourse core's
last 5000 commits index in about fifteen seconds.

`blame` is the second pass: for every fix commit it asks git who wrote
the lines the fix removed, so `fixes` pairs each fix with the change it
corrected (weighted by lines) and `undone <sha>` lists the later fixes
that corrected a given commit. Pairing on lines rather than shared files
is what keeps a large refactor from pairing with everything after it.

Register with Claude Code:

```
claude mcp add hindsight -- /path/to/hindsight serve
```

The server exposes three read-only tools: `search` (term search, all-terms
matches rank first), `get_page` (read one page), `list` (index or a
directory). No LLM inside; the calling agent does the reasoning.

## Roadmap

- [x] Corpus format and schema (AGENTS.md)
- [x] Ingest the backlog of raw incidents, prove the format
- [x] `hindsight init` - scaffold a corpus directory
- [x] MCP server (stdio): `search`, `get_page`, `list` over a corpus
- [x] `hindsight extract`: git history as a queryable raw index
- [x] `hindsight git blame`: correction pairs by line provenance
- [ ] Remote transport (HTTP) + auth, for hosting a central corpus
- [ ] Captain Hindsight: alert-driven investigation agent built on top
