---
id: backend-005
title: Simplify Configuration
status: todo
priority: medium
tags:
- setup
- fix
dependencies:
- backend-002
assignee: developer
created: 2025-11-29T18:47:37.562154283Z
estimate: 2h
complexity: 4
area: backend
---

**Priority:** MEDIUM
**Effort:** 2 hours
**Area:** backend
**Dependencies:** [backend-002]

Reduce configuration complexity for library consumers.

#### Current State
Config has many options for full mitmproxy compatibility. Library users need simple setup.

#### Implementation
```rust
let proxy = MitmproxyBuilder::new()
    .listen_port(8080)
    .ca_cert_path("~/.mitmproxy/ca.pem")
    .interceptor(my_interceptor)
    .build()?;

proxy.run().await?;
```

#### Acceptance Criteria
- [ ] Builder with sensible defaults
- [ ] Full config still available for advanced users
- [ ] Example showing minimal setup
- [ ] Auto-generate CA cert if not provided

---