# Hindsight Data Structure

## Overview

After analyzing 35 incident topics, here's the proposed structure for making incident data searchable and useful for investigation agents.

## Current Structure (v1 - Flat JSON)

Each incident is stored as `topic-{id}.json` with:

```json
{
  "topic_id": 185467,
  "title": "...",
  "url": "...",
  "created_at": "2026-06-07T15:36:25.544Z",
  "alert_type": "hardware_failure|ddos|resource_exhaustion|deployment_issue|...",
  "affected_systems": ["ta03.yyz2", "flex032"],
  "symptoms": ["read-only mode", "NIC timeout"],
  "investigation_steps": ["Checked machine logs", "Rebooted via IPMI"],
  "root_cause": "...",
  "resolution": "...",
  "duration": "18 minutes",
  "raw_content": "..."
}
```

### Strengths
- Simple, flat structure
- Easy to parse and serialize
- Human-readable

### Weaknesses
- No structured timeline (just flat array of steps)
- Investigation steps lack context (who ran what command when)
- Symptoms not linked to affected systems
- No failure patterns extracted
- No relationships between incidents

## Proposed Structure (v2 - Structured for Agent Use)

### Core Fields (keep from v1)
```json
{
  "topic_id": 185467,
  "title": "...",
  "url": "...",
  "created_at": "2026-06-07T15:36:25.544Z",
  "updated_at": "2026-06-07T15:36:25.544Z"
}
```

### Alert Classification (enhanced)
```json
{
  "alert": {
    "type": "hardware_failure",
    "category": "infrastructure",  // infrastructure, application, network, security
    "severity": "critical",  // critical, major, minor
    "tags": ["nic-failure", "broadcom", "bnxt_en", "recurring"]
  }
}
```

### Affected Systems (structured)
```json
{
  "affected_systems": [
    {
      "type": "host",  // host, cluster, service, fleet
      "name": "ta03.yyz2",
      "role": "postgres_host",  // postgres_host, redis_host, app_cluster, load_balancer
      "impact": "complete_failure"  // complete_failure, degraded, read_only
    },
    {
      "type": "cluster",
      "name": "flex032",
      "role": "app_cluster",
      "impact": "read_only"
    }
  ]
}
```

### Symptoms (with context)
```json
{
  "symptoms": [
    {
      "description": "sites in read-only mode",
      "user_facing": true,
      "systems": ["flex032", "flex036", "flex050"],
      "first_observed": "2026-06-04T16:01:00Z"
    },
    {
      "description": "NIC timeout errors in kernel logs",
      "user_facing": false,
      "systems": ["ta03.yyz2"],
      "error_message": "bnxt_en 0000:ca:00.0 eth0: TX timeout detected",
      "first_observed": "2026-06-04T16:01:00Z"
    }
  ]
}
```

### Investigation Timeline (structured)
```json
{
  "timeline": [
    {
      "timestamp": "2026-06-04T16:01:00Z",
      "event_type": "symptom_detected",
      "description": "NIC timeout error in logs",
      "actor": "automated_monitoring"
    },
    {
      "timestamp": "2026-06-04T16:05:00Z",
      "event_type": "investigation_action",
      "description": "Checked machine logs for NIC errors",
      "command": "dmesg | grep bnxt_en",
      "actor": "fitzy"
    },
    {
      "timestamp": "2026-06-04T16:10:00Z",
      "event_type": "mitigation_action",
      "description": "Rebooted machine via IPMI/doobs",
      "command": "doobs reboot ta03.yyz2",
      "actor": "fitzy"
    },
    {
      "timestamp": "2026-06-04T16:18:00Z",
      "event_type": "recovery",
      "description": "Services restored",
      "actor": "automated"
    }
  ]
}
```

### Root Cause (structured)
```json
{
  "root_cause": {
    "summary": "Broadcom bnxt_en NIC driver timeout",
    "category": "hardware_failure",
    "component": "network_interface_card",
    "failure_mode": "driver_timeout",
    "contributing_factors": [
      "Broadcom NIC driver bug",
      "Both ethernet ports failed simultaneously",
      "Hardware-level fault"
    ],
    "error_messages": [
      "bnxt_en 0000:ca:00.0 eth0: TX timeout detected, starting reset task!",
      "bnxt_en 0000:ca:00.1 eth1: TX timeout detected, starting reset task!"
    ],
    "related_incidents": [185229, 164479]  // Other bnxt_en failures
  }
}
```

### Resolution (structured)
```json
{
  "resolution": {
    "summary": "Rebooted and moved all services off failing hardware",
    "actions": [
      {
        "description": "Reboot machine",
        "command": "doobs reboot ta03.yyz2",
        "result": "temporary recovery"
      },
      {
        "description": "Move Postgres primaries",
        "systems_affected": ["flex032", "flex036", "flex050"],
        "result": "partial mitigation"
      },
      {
        "description": "Move Redis primaries",
        "systems_affected": ["flex032", "flex036", "flex050"],
        "result": "complete mitigation"
      }
    ],
    "permanent_fix": "Hardware replacement required",
    "lessons_learned": [
      "Incomplete service migration causes recurring incidents",
      "Need checklist: Postgres, Redis, Apps, Nomad all must be moved"
    ]
  }
}
```

### Impact Metrics
```json
{
  "impact": {
    "duration": "18 minutes",
    "duration_seconds": 1080,
    "user_facing_downtime": "18 minutes",
    "sites_affected": 3,
    "customers_affected": 3,
    "recurrence": {
      "is_recurring": true,
      "previous_incidents": [185229],
      "recurred_within": "24 hours",
      "reason": "incomplete remediation"
    }
  }
}
```

### Failure Pattern (extracted)
```json
{
  "pattern": {
    "name": "broadcom_nic_failure",
    "signature": {
      "symptoms": ["NIC timeout", "both network ports fail", "SSH unresponsive"],
      "error_patterns": ["bnxt_en.*TX timeout detected"],
      "affected_hardware": ["Broadcom NIC with bnxt_en driver"]
    },
    "detection": {
      "check": "dmesg | grep 'bnxt_en.*TX timeout'",
      "monitoring": "NIC error rate metrics"
    },
    "response_playbook": {
      "immediate": [
        "Reboot via IPMI/doobs (SSH will be down)",
        "Unlock LUKS disk if needed"
      ],
      "mitigation": [
        "Move ALL services off host:",
        "- Postgres primaries",
        "- Redis primaries", 
        "- Application clusters",
        "- Nomad allocations"
      ],
      "long_term": "Hardware replacement"
    }
  }
}
```

### Searchable Metadata
```json
{
  "metadata": {
    "keywords": ["nic", "broadcom", "bnxt_en", "timeout", "hardware", "network"],
    "error_patterns": ["bnxt_en.*TX timeout", "eth[0-9].*timeout"],
    "commands_used": [
      "dmesg | grep bnxt_en",
      "doobs reboot",
      "puppet-move-postgres-primary"
    ],
    "related_topics": [185229, 164479],
    "embedding": null  // For semantic search later
  }
}
```

## Use Cases for Captain Hindsight

### 1. Alert Matching
When new alert fires with "NIC timeout on ta05.yyz2":
- Search `symptoms` for "NIC timeout"
- Search `error_patterns` for matching regex
- Find incidents 185467, 185229, 164479

### 2. Investigation Guidance
Agent sees past incidents and knows:
- Check: `dmesg | grep bnxt_en`
- Expect: Both eth0 and eth1 to fail
- Action: Reboot via IPMI (SSH will be down)
- Follow-up: Move ALL services (not just Postgres!)

### 3. Pattern Recognition
Agent identifies this is a recurring pattern:
- Same failure mode (Broadcom NIC)
- Same driver (bnxt_en)
- Known incomplete remediation risk
- Suggests complete service migration checklist

### 4. Semantic Search (future)
- Embed incident descriptions
- Find similar incidents even with different wording
- "network interface frozen" → finds "NIC timeout" incidents

## Migration Path

### Phase 1: Keep v1, add indexes
- Keep flat JSON structure
- Add `hindsight/data/indexes/`:
  - `by-alert-type.json` - group by alert_type
  - `by-system.json` - group by affected systems
  - `keywords.json` - inverted index for text search

### Phase 2: Gradual v2 adoption
- Write v2 parser for new incidents
- Keep v1 for existing incidents
- Agent can read both formats

### Phase 3: Full v2 migration
- Reparse all v1 incidents to v2
- Extract failure patterns
- Build semantic search embeddings

## Files Structure

```
hindsight/
├── data/
│   ├── raw/              # Original flat JSON (v1)
│   │   └── topic-*.json
│   ├── structured/       # Enhanced JSON (v2)
│   │   └── incident-*.json
│   ├── patterns/         # Extracted failure patterns
│   │   ├── broadcom-nic-failure.json
│   │   ├── ddos-response.json
│   │   └── redis-oom.json
│   └── indexes/
│       ├── by-alert-type.json
│       ├── by-system.json
│       ├── keywords.json
│       └── embeddings.bin  # For semantic search
```
