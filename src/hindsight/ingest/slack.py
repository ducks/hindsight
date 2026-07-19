"""Slack incident ingestion."""

from typing import List
from pathlib import Path

from .base import IncidentIngester, Incident


class SlackIngester(IncidentIngester):
    """Ingest incidents from Slack #incident channels."""

    def __init__(self, output_dir: Path, slack_token: str = None):
        super().__init__(output_dir)
        self.slack_token = slack_token

    def fetch_incidents(self, channel: str = "incident", **kwargs) -> List[Incident]:
        """Fetch incident threads from Slack channel."""
        raise NotImplementedError("Slack ingestion not yet implemented")

    def parse_incident(self, raw_thread: dict) -> Incident:
        """Parse Slack thread into Incident."""
        raise NotImplementedError("Slack ingestion not yet implemented")
