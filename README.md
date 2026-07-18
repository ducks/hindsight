# hindsight

Incident knowledge base - learning from past production incidents to improve future investigations.

Powers **Captain Hindsight**, the automated investigation agent who always knows what went wrong... after the fact.

## Part of the Incident Response Ecosystem

- **hindsight** (this repo) - learns from real production incidents, builds searchable knowledge base
- **Captain Hindsight** - investigation agent that uses hindsight data to diagnose new alerts
- **[replaybook](../replaybook/)** - turns incidents into playable training scenarios

## Vision

Build a searchable corpus of past incidents to power automated investigation agents. Every production incident becomes institutional knowledge that can:
1. Help Captain Hindsight investigate new alerts
2. Generate replaybook training scenarios
3. Surface patterns and failure modes

## Phase 1: Data Ingestion

Extract incident data from internal incident tracking and structure for retrieval.

### Data Sources

- Topics tagged `#incident` or `#alert` 
- Topics in incident-related categories
- Alert topics from monitoring systems

### Data Structure

Each incident record contains:
- Alert metadata (cluster, service, metric, severity)
- Timeline of investigation steps
- Root cause analysis
- Resolution steps
- Related topics/incidents

### Data Format

Incidents are stored in `data/incidents/YYYYMMDD-*.json` using date-ver versioning. Each file contains structured incident data with:
- Metadata (topic_id, url, created_at, severity)
- Timeline with timestamped events
- Failure patterns with playbooks
- Searchable keywords and error patterns

See `docs/data-structure.md` for the full v2 schema.

## Roadmap

- [x] Phase 1: Ingest incident topics
- [x] Phase 2: Parse and structure incident data (v2 format with date-ver)
- [ ] Phase 3: Build semantic search (embeddings + vector DB)
- [ ] Phase 4: API for investigation agent integration
- [ ] Phase 5: Continuous learning (update from new incidents)
- [ ] Phase 6: Generate replaybook scenarios from incidents
