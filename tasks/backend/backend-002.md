---
id: backend-002
title: Create Library Feature Flags
status: todo
priority: high
tags:
- setup
- fix
dependencies: []
assignee: developer
created: 2025-11-29T18:47:37.562103659Z
estimate: 4h
complexity: 4
area: backend
---

**Priority:** HIGH
**Effort:** 4 hours
**Area:** backend

Restructure Cargo.toml with feature flags for modular usage.

#### Current State
All features are always compiled. HalluciGuard doesn't need REST API or WebSocket UI.

#### Implementation
```toml
[features]
default = ["http-proxy", "tls-intercept"]

# Core (always needed)
http-proxy = []
tls-intercept = ["openssl", "rcgen"]

# Optional
rest-api = ["axum", "tower"]
websocket-api = ["tokio-tungstenite"]
sse-parsing = []

# Not needed for HalluciGuard
flow-filtering = ["regex"]
addon-system = []
```

#### Acceptance Criteria
- [ ] Minimal build compiles without REST API deps
- [ ] HalluciGuard can import with only needed features
- [ ] `cargo build --no-default-features --features "http-proxy,tls-intercept,sse-parsing"` works
- [ ] Document feature combinations in README

---