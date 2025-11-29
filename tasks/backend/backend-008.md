---
id: backend-008
title: Implement BufferedH2Connection
status: todo
priority: critical
tags:
- setup
- fix
dependencies:
- backend-006
- backend-007
assignee: developer
created: 2025-11-29T18:53:18.868997663Z
estimate: 6h
complexity: 4
area: backend
---

**Priority:** CRITICAL
**Effort:** 6 hours
**Area:** backend
**Dependencies:** [backend-006, backend-007]

Complete the BufferedH2Connection implementation for HTTP/2 support.

#### Problem
The `receive_data()` method is incomplete, causing HTTP/2 to fail.

#### Implementation
```rust
impl BufferedH2Connection {
    pub fn receive_data(&mut self, data: &[u8]) -> Result<Vec<H2Event>, ProxyError> {
        // Use h2 crate's public API only (NOT h2::frame)
        // Must parse incoming bytes and return structured events
    }
}
```

#### Reference
Python implementation: `mitmproxy/proxy/layers/http/_http_h2.py:122-125`

#### Acceptance Criteria
- [ ] `receive_data()` parses h2 frames correctly
- [ ] Returns structured H2Event types
- [ ] Uses only public h2 crate API
- [ ] Unit tests with sample HTTP/2 frames

---