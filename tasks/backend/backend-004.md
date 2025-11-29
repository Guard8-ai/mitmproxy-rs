---
id: backend-004
title: Remove Unnecessary Components
status: todo
priority: medium
tags:
- setup
- fix
dependencies: []
assignee: developer
created: 2025-11-29T18:47:37.562143143Z
estimate: 2h
complexity: 4
area: backend
---

**Priority:** MEDIUM
**Effort:** 2 hours
**Area:** backend

Remove or gate components not needed for library use.

#### Components to Remove/Gate
Behind `addon-system` feature (disabled by default):
- Command execution framework (src/api/commands)
[ ] Addon loading system

Behind `flow-ui` feature (disabled by default):
- Flow filtering UI helpers
- Flow marking/comments (keep data structures, remove UI)

Keep but simplify:
- Flow storage (make optional, can use external storage)
- WebSocket API (useful for monitoring, keep behind feature)

#### Acceptance Criteria
- [ ] Default build is minimal
- [ ] Removed code doesn't break core functionality
- [ ] Binary size reduced for minimal builds
- [ ] Document what each feature adds

---