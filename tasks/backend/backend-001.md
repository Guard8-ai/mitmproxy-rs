---
id: backend-001
title: Add SSE Stream Parsing Module
status: todo
priority: critical
tags:
- setup
- fix
dependencies: []
assignee: developer
created: 2025-11-29T18:47:37.562088370Z
estimate: ~
complexity: 4
area: backend
---

**Priority:** CRITICAL
**Effort:** 1 day
**Area:** backend

Implement Server-Sent Events parsing for LLM API responses (Claude, OpenAI, etc.).

#### Context
Currently HTTP responses are captured as raw bodies. LLM APIs use SSE format:
```
data: {"type": "content_block_delta", "delta": {"text": "Hello"}}

data: {"type": "message_stop"}

data: [DONE]
```

#### Implementation
Create `src/sse.rs`:
- Parse line-based SSE format
- Extract `data:`, `event:`, `id:`, `retry:` fields
- Handle multi-line data values
- Buffer incomplete events across chunks
- Return iterator of parsed events

#### Acceptance Criteria
- [ ] Parse standard SSE format (data, event, id, retry fields)
- [ ] Handle chunked/streaming responses
- [ ] Extract JSON payloads from data fields
- [ ] Unit tests with real Claude API response samples

---