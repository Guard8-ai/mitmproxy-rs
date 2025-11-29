---
id: backend-003
title: Add Response Interception Hook
status: todo
priority: high
tags:
- setup
- fix
dependencies:
- backend-001
assignee: developer
created: 2025-11-29T18:47:37.562129158Z
estimate: 6h
complexity: 4
area: backend
---

**Priority:** HIGH
**Effort:** 6 hours
**Area:** backend
**Dependencies:** [backend-001]

Allow consumers to intercept and process HTTP responses before forwarding.

#### Context
HalluciGuard needs to:
1. Intercept response body
2. Parse SSE events
3. Run pattern detection
4. Optionally modify/block

#### Implementation
```rust
pub trait ResponseInterceptor: Send + Sync {
    fn on_response(&self, flow: &mut HTTPFlow) -> InterceptResult;
    fn on_response_chunk(&self, flow: &HTTPFlow, chunk: &[u8]) -> ChunkResult;
}

pub enum InterceptResult {
    Continue,
    Modify(HTTPResponse),
    Block,
}
```

#### Acceptance Criteria
- [ ] Hook called for every HTTP response
- [ ] Streaming responses call on_response_chunk per chunk
- [ ] Can modify response before forwarding
- [ ] Can block responses entirely
- [ ] Async support for slow interceptors

---