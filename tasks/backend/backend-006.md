---
id: backend-006
title: Remove h2::Reason Usage
status: todo
priority: critical
tags:
- setup
- fix
dependencies: []
assignee: developer
created: 2025-11-29T18:53:18.868982220Z
estimate: 2h
complexity: 4
area: backend
---

**Priority:** CRITICAL
**Effort:** 2 hours
**Area:** backend

Replace ALL `h2::Reason` references with numeric error codes throughout the codebase.

#### Problem
The h2 crate's `Reason` type is being used incorrectly, causing compilation failures.

#### Solution
```rust
// REMOVE patterns like:
h2::Reason::CANCEL
h2::Reason::PROTOCOL_ERROR

// REPLACE WITH numeric codes:
0x8  // CANCEL
0x1  // PROTOCOL_ERROR
0x0  // NO_ERROR
0x2  // INTERNAL_ERROR
0x3  // FLOW_CONTROL_ERROR
0x4  // SETTINGS_TIMEOUT
0x5  // STREAM_CLOSED
0x6  // FRAME_SIZE_ERROR
0x7  // REFUSED_STREAM
0x9  // COMPRESSION_ERROR
0xa  // CONNECT_ERROR
0xb  // ENHANCE_YOUR_CALM
0xc  // INADEQUATE_SECURITY
0xd  // HTTP_1_1_REQUIRED
```

#### Acceptance Criteria
- [ ] No `h2::Reason::` patterns in codebase
- [ ] All error codes use numeric constants
- [ ] Define constants in a central location

---