"""Alertmanager incident ingestion."""

from typing import List
from pathlib import Path

from .base import IncidentIngester, Incident


class AlertmanagerIngester(IncidentIngester):
    """Ingest resolved alerts from Alertmanager."""

    def __init__(self, output_dir: Path, alertmanager_url: str = None):
        super().__init__(output_dir)
        self.alertmanager_url = alertmanager_url

    def fetch_incidents(self, lookback_days: int = 7, **kwargs) -> List[Incident]:
        """Fetch resolved alerts from Alertmanager."""
        raise NotImplementedError("Alertmanager ingestion not yet implemented")

    def parse_incident(self, raw_alert: dict) -> Incident:
        """Parse Alertmanager alert into Incident."""
        raise NotImplementedError("Alertmanager ingestion not yet implemented")
