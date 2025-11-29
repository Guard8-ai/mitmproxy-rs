---
id: backend-007
title: Convert Async Methods to Sync CommandGenerator
status: todo
priority: critical
tags:
- setup
- fix
dependencies:
- backend-006
assignee: developer
created: 2025-11-29T18:53:18.868991770Z
estimate: 4h
complexity: 4
area: backend
---

**Priority:** CRITICAL
**Effort:** 4 hours
**Area:** backend
**Dependencies:** [backend-006]

Convert all remaining async methods to synchronous CommandGenerator pattern.

#### Problem
Mixed async/sync architecture causes type mismatches and compilation errors.

#### Methods to Convert
- `protocol_error()`
- `handle_response_data()`
- `handle_response_end()`
- `handle_response_error()`
- All HTTP/2 stream handlers

#### Pattern
```rust
// OLD (async):
async fn method(&mut self, param: Type) -> Result<Vec<Box<dyn Command>>, ProxyError>

// NEW (sync):
fn method(&mut self, param: Type) -> Box<dyn CommandGenerator<()>>
```

#### Acceptance Criteria
- [ ] Zero `async fn` in proxy layers
- [ ] All methods return `Box<dyn CommandGenerator<()>>`
- [ ] No `.await` usage in layer code

---