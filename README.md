# hindsight

Incident knowledge base - learning from past production incidents to improve future investigations.

Powers **Captain Hindsight**, the automated investigation agent who always knows what went wrong... after the fact.

## Vision

Build a searchable corpus of past incidents from dev.discourse.org to power automated investigation agents. Every production incident becomes institutional knowledge.

## Phase 1: Data Ingestion

Extract incident data from dev.discourse.org topics and structure for retrieval.

### Data Sources

- Topics tagged `#incident` or `#alert` 
- Topics in incident-related categories
- Alert topics from Prometheus/Alertmanager

### Data Structure

Each incident record contains:
- Alert metadata (cluster, service, metric, severity)
- Timeline of investigation steps
- Root cause analysis
- Resolution steps
- Related topics/incidents

### Usage

```bash
# Ingest incidents from dev.discourse.org
bin/ingest-incidents

# Search for similar incidents
bin/search-incidents "high error rate cluster-foo"
```

## Roadmap

- [ ] Phase 1: Ingest incident topics from dev.discourse.org
- [ ] Phase 2: Parse and structure incident data
- [ ] Phase 3: Build semantic search (embeddings + vector DB)
- [ ] Phase 4: API for investigation agent integration
- [ ] Phase 5: Continuous learning (update from new incidents)
