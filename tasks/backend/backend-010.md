---
id: backend-010
title: Fix Event Trait Methods
status: todo
priority: high
tags:
- setup
- fix
dependencies: []
assignee: developer
created: 2025-11-29T18:53:18.869011580Z
estimate: 2h
complexity: 4
area: backend
---

**Priority:** HIGH
**Effort:** 2 hours
**Area:** backend

Add missing Event trait method implementations.

#### Problem
Event types are missing required trait methods, causing compilation errors.

#### Acceptance Criteria
- [ ] All Event types implement required methods
- [ ] Event serialization/deserialization works
- [ ] Event matching in handlers compiles

---

## Summary

| Priority | Task | Effort | Blocks |
|----------|------|--------|--------|
| CRITICAL | Remove h2::Reason | 2h | backend-007, backend-008 |
| CRITICAL | Async to Sync | 4h | backend-008, backend-009 |
| CRITICAL | BufferedH2Connection | 6h | - |
| HIGH | Layer Trait | 3h | - |
| HIGH | Event Trait | 2h | - |

**Total Effort:** ~17 hours

**Order:**
1. Remove h2::Reason (unblocks many errors)
2. Async to Sync conversion
3. Layer trait fixes
4. Event trait fixes
5. BufferedH2Connection (depends on above)

---

## Success Criteria

- [ ] `cargo check` passes with 0 errors
- [ ] All async methods converted to CommandGenerator
- [ ] No h2::Reason usage in codebase
- [ ] All layers implement Layer trait correctly
- [ ] BufferedH2Connection fully functional