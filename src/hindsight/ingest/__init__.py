"""Incident ingestion from multiple sources."""

from .base import IncidentIngester, Incident
from .discourse import DiscourseIngester

__all__ = ["IncidentIngester", "Incident", "DiscourseIngester"]
