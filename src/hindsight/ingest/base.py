"""Base classes for incident ingestion."""

from abc import ABC, abstractmethod
from dataclasses import dataclass, asdict
from datetime import datetime
from typing import Optional, List, Dict, Any
import json
from pathlib import Path


@dataclass
class Incident:
    """Structured incident data (v2 format)."""

    # Metadata
    source: str  # 'discourse', 'slack', 'alertmanager'
    source_id: str  # topic_id, thread_ts, alert_id
    url: str
    title: str
    created_at: str  # ISO8601
    severity: Optional[str] = None
    alert_type: Optional[str] = None
    affected_systems: List[str] = None

    # Timeline
    detection_time: Optional[str] = None
    investigation_start: Optional[str] = None
    root_cause_identified: Optional[str] = None
    resolution_start: Optional[str] = None
    resolution_complete: Optional[str] = None
    timeline_events: List[Dict[str, Any]] = None

    # Investigation
    symptoms: Optional[str] = None
    investigation_steps: List[str] = None
    commands_run: List[str] = None
    tools_used: List[str] = None
    dead_ends: List[str] = None

    # Resolution
    root_cause: Optional[str] = None
    fix_applied: Optional[str] = None
    preventive_measures: List[str] = None

    # Failure patterns
    failure_category: Optional[str] = None
    failure_mode: Optional[str] = None
    triggers: List[str] = None
    playbook_ref: Optional[str] = None

    # Impact
    duration_minutes: Optional[int] = None
    affected_users: Optional[int] = None
    service_degradation: Optional[str] = None

    # Searchable
    keywords: List[str] = None
    error_patterns: List[str] = None
    similar_incidents: List[str] = None

    # Raw
    raw_content: Optional[str] = None

    def __post_init__(self):
        """Initialize mutable defaults."""
        if self.affected_systems is None:
            self.affected_systems = []
        if self.timeline_events is None:
            self.timeline_events = []
        if self.investigation_steps is None:
            self.investigation_steps = []
        if self.commands_run is None:
            self.commands_run = []
        if self.tools_used is None:
            self.tools_used = []
        if self.dead_ends is None:
            self.dead_ends = []
        if self.preventive_measures is None:
            self.preventive_measures = []
        if self.triggers is None:
            self.triggers = []
        if self.keywords is None:
            self.keywords = []
        if self.error_patterns is None:
            self.error_patterns = []
        if self.similar_incidents is None:
            self.similar_incidents = []

    def to_dict(self) -> Dict[str, Any]:
        """Convert to JSON structure."""
        return {
            "metadata": {
                "source": self.source,
                "source_id": self.source_id,
                "url": self.url,
                "title": self.title,
                "created_at": self.created_at,
                "severity": self.severity,
                "alert_type": self.alert_type,
                "affected_systems": self.affected_systems,
            },
            "timeline": {
                "detection": self.detection_time or self.created_at,
                "investigation_start": self.investigation_start or self.created_at,
                "root_cause_identified": self.root_cause_identified,
                "resolution_start": self.resolution_start,
                "resolution_complete": self.resolution_complete,
                "events": self.timeline_events,
            },
            "investigation": {
                "symptoms": self.symptoms,
                "steps": self.investigation_steps,
                "commands_run": self.commands_run,
                "tools_used": self.tools_used,
                "dead_ends": self.dead_ends,
            },
            "resolution": {
                "root_cause": self.root_cause,
                "fix_applied": self.fix_applied,
                "preventive_measures": self.preventive_measures,
            },
            "failure_patterns": [
                {
                    "category": self.failure_category,
                    "failure_mode": self.failure_mode,
                    "triggers": self.triggers,
                    "playbook_ref": self.playbook_ref,
                }
            ] if self.failure_category else [],
            "impact": {
                "duration_minutes": self.duration_minutes,
                "affected_users": self.affected_users,
                "service_degradation": self.service_degradation,
            },
            "searchable": {
                "keywords": self.keywords,
                "error_patterns": self.error_patterns,
                "similar_incidents": self.similar_incidents,
            },
            "raw": {
                "original_content": self.raw_content,
            },
        }

    def filename(self) -> str:
        """Generate date-ver filename: YYYYMMDD-source-id.json"""
        dt = datetime.fromisoformat(self.created_at.replace('Z', '+00:00'))
        date_prefix = dt.strftime('%Y%m%d')
        return f"{date_prefix}-{self.source_id}.json"


class IncidentIngester(ABC):
    """Base class for ingesting incidents from various sources."""

    def __init__(self, output_dir: Path):
        self.output_dir = Path(output_dir)
        self.output_dir.mkdir(parents=True, exist_ok=True)

    @abstractmethod
    def fetch_incidents(self, **kwargs) -> List[Incident]:
        """Fetch incidents from the source. Returns list of Incident objects."""
        pass

    @abstractmethod
    def parse_incident(self, raw_data: Any) -> Incident:
        """Parse raw incident data into structured Incident object."""
        pass

    def save_incident(self, incident: Incident) -> Path:
        """Save incident to disk as JSON."""
        filepath = self.output_dir / incident.filename()
        with open(filepath, 'w') as f:
            json.dump(incident.to_dict(), f, indent=2)
        return filepath

    def ingest(self, **kwargs) -> List[Path]:
        """Main ingestion flow: fetch, parse, save."""
        incidents = self.fetch_incidents(**kwargs)
        saved_files = []

        for incident in incidents:
            filepath = self.save_incident(incident)
            saved_files.append(filepath)
            print(f"✓ Saved {filepath.name}")

        return saved_files
