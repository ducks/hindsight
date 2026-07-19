export const meta = {
  name: 'ingest-incidents',
  description: 'Fetch incident topics from Discourse and ingest into hindsight',
  phases: [
    { title: 'Fetch', detail: 'Get topics via MCP' },
    { title: 'Ingest', detail: 'Parse and save to data/incidents/' }
  ]
}

const INCIDENT_SCHEMA = {
  type: "object",
  properties: {
    id: { type: "number" },
    title: { type: "string" },
    created_at: { type: "string" },
    posts: { type: "array", items: { type: "string" } }
  },
  required: ["id", "title", "created_at", "posts"]
}

phase('Fetch')

// Get topic IDs
const filterResult = await agent('Use discourse_filter_topics MCP tool with tag:postmortem, get 10 topics. Return the topic IDs.', {
  label: 'filter-topics',
  schema: {
    type: "object",
    properties: {
      topic_ids: { type: "array", items: { type: "number" } }
    },
    required: ["topic_ids"]
  }
})

log(`Found ${filterResult.topic_ids.length} topics to ingest`)

// Fetch each topic
phase('Ingest')
const topics = await parallel(filterResult.topic_ids.slice(0, 5).map(topicId => () =>
  agent(`Use discourse_read_topic MCP tool for topic ${topicId}. Extract: id, title, created_at, and concatenate all post.raw content into a posts array. Return as JSON.`, {
    label: `topic-${topicId}`,
    schema: INCIDENT_SCHEMA
  })
))

// Save to file for Python ingester
const topicsJson = JSON.stringify(topics.filter(Boolean), null, 2)
await agent(`Write this JSON to /tmp/hindsight-topics.json: ${topicsJson}`, {
  label: 'save-json'
})

// Run Python ingester
await agent(`Run: cd ~/dev/hindsight && nix-shell --run 'python3 -c "
import sys, json
from pathlib import Path
sys.path.insert(0, \"src\")
from hindsight.ingest.discourse import DiscourseIngester

with open(\"/tmp/hindsight-topics.json\") as f:
    topics = json.load(f)

ingester = DiscourseIngester(Path(\"data/incidents\"))
for t in topics:
    t[\"base_url\"] = \"https://dev.discourse.org\"
    fp = ingester.save_incident(ingester.parse_incident(t))
    print(f\"✓ {fp.name}\")
"'`, {
  label: 'ingest'
})

return { ingested: topics.length }
