"""Discourse incident ingestion via MCP."""

from typing import List, Any
from pathlib import Path

from .base import IncidentIngester, Incident


class DiscourseIngester(IncidentIngester):
    """Ingest incidents from Discourse via MCP server."""

    def __init__(self, output_dir: Path, mcp_client=None):
        super().__init__(output_dir)
        self.mcp = mcp_client  # MCP client for discourse-dev server

    def fetch_incidents(self, tags: List[str] = None, limit: int = 50) -> List[Incident]:
        """
        Fetch incident topics from Discourse.

        This method is designed to be called from Claude Code context where
        MCP tools are available. In standalone mode, pass raw topic data.
        """
        if tags is None:
            tags = ['incident', 'alert', 'outage']

        # When called from Claude Code, the agent should use MCP tools directly
        # and pass the results to parse_incident()
        raise NotImplementedError(
            "fetch_incidents should be called from Claude Code agent context "
            "using discourse MCP tools (discourse_filter_topics, discourse_read_topic)"
        )

    def parse_incident(self, raw_topic: dict) -> Incident:
        """
        Parse a Discourse topic into structured Incident.

        Args:
            raw_topic: Dict with keys: id, title, created_at, posts (list of post content)
        """
        content = "\n\n".join(raw_topic.get('posts', []))

        # Extract metadata
        title = raw_topic['title']
        topic_id = str(raw_topic['id'])
        created_at = raw_topic['created_at']

        # Basic keyword extraction
        keywords = self._extract_keywords(content)
        error_patterns = self._extract_error_patterns(content)
        failure_category = self._classify_failure(content)

        # Extract base URL from raw_topic if provided, otherwise leave generic
        base_url = raw_topic.get('base_url', '')
        url = f"{base_url}/t/{topic_id}" if base_url else f"/t/{topic_id}"

        return Incident(
            source='discourse',
            source_id=topic_id,
            url=url,
            title=title,
            created_at=created_at,
            symptoms=self._extract_symptoms(content),
            root_cause=self._extract_root_cause(content),
            fix_applied=self._extract_resolution(content),
            keywords=keywords,
            error_patterns=error_patterns,
            failure_category=failure_category,
            raw_content=content,
        )

    def _extract_keywords(self, content: str) -> List[str]:
        """Extract searchable keywords from content."""
        keywords = []
        content_lower = content.lower()

        # Common patterns
        if 'high error' in content_lower or 'error rate' in content_lower:
            keywords.append('high-error-rate')
        if 'timeout' in content_lower:
            keywords.append('timeout')
        if any(x in content_lower for x in ['500', '502', '503', '504']):
            keywords.append('5xx')
        if 'memory' in content_lower or 'oom' in content_lower:
            keywords.append('memory')
        if 'disk' in content_lower or 'space' in content_lower:
            keywords.append('disk')
        if 'network' in content_lower or 'connection' in content_lower:
            keywords.append('network')
        if 'redis' in content_lower:
            keywords.append('redis')
        if 'postgres' in content_lower:
            keywords.append('postgres')
        if 'sidekiq' in content_lower:
            keywords.append('sidekiq')
        if 'ddos' in content_lower or 'attack' in content_lower:
            keywords.append('ddos')

        return keywords

    def _extract_error_patterns(self, content: str) -> List[str]:
        """Extract error message patterns."""
        import re
        patterns = []

        # Look for error/exception patterns
        matches = re.findall(r'(?:error|exception|failed):\s*(.+?)(?:\n|$)', content, re.IGNORECASE)
        patterns.extend([m.strip() for m in matches[:5]])  # Top 5

        return patterns

    def _classify_failure(self, content: str) -> str:
        """Classify failure type from content."""
        content_lower = content.lower()

        if any(x in content_lower for x in ['nic', 'driver', 'hardware', 'disk', 'raid']):
            return 'hardware'
        if any(x in content_lower for x in ['network', 'bgp', 'routing', 'dns']):
            return 'network'
        if any(x in content_lower for x in ['memory', 'oom', 'disk full', 'space']):
            return 'resource-exhaustion'
        if any(x in content_lower for x in ['deploy', 'release', 'rollout']):
            return 'deployment'
        if any(x in content_lower for x in ['config', 'setting', 'env var']):
            return 'configuration'
        if any(x in content_lower for x in ['ddos', 'attack', 'abuse']):
            return 'external'
        if any(x in content_lower for x in ['bug', 'code', 'logic']):
            return 'application'

        return 'unknown'

    def _extract_symptoms(self, content: str) -> str:
        """Extract symptom description from first few paragraphs."""
        paragraphs = [p.strip() for p in content.split('\n\n') if p.strip()]
        return paragraphs[0] if paragraphs else ""

    def _extract_root_cause(self, content: str) -> str:
        """Extract root cause if mentioned."""
        import re
        # Look for "root cause" mentions
        match = re.search(r'root cause[:\s]+(.+?)(?:\n\n|\n#|$)', content, re.IGNORECASE | re.DOTALL)
        if match:
            return match.group(1).strip()

        # Look for "cause:" or "caused by:"
        match = re.search(r'caused? by[:\s]+(.+?)(?:\n\n|\n#|$)', content, re.IGNORECASE | re.DOTALL)
        if match:
            return match.group(1).strip()

        return None

    def _extract_resolution(self, content: str) -> str:
        """Extract resolution/fix description."""
        import re
        # Look for resolution mentions
        match = re.search(r'(?:resolution|fix|solved)[:\s]+(.+?)(?:\n\n|\n#|$)', content, re.IGNORECASE | re.DOTALL)
        if match:
            return match.group(1).strip()

        return None
