---
id: setup-001
title: Update Documentation
status: todo
priority: low
tags:
- setup
- fix
dependencies:
- backend-002
- api-001
assignee: developer
created: 2025-11-29T18:47:37.562176861Z
estimate: 2h
complexity: 4
area: setup
---

**Priority:** LOW
**Effort:** 2 hours
**Area:** setup
**Dependencies:** [backend-002, api-001]

Update README for library usage focus.

#### Changes
[ ] Add "Using as a Library" section
- Show HalluciGuard-style integration example
- Document feature flags
[ ] Remove/minimize full mitmproxy clone focus
[ ] Add API documentation

#### Acceptance Criteria
- [ ] README shows library usage first
- [ ] Feature flags documented
- [ ] Example code compiles
- [ ] Link to HalluciGuard as reference implementation

---

## Summary

| Priority | Task | Effort | Area |
|----------|------|--------|------|
| CRITICAL | SSE Stream Parsing | 1 day | backend |
| HIGH | Feature Flags | 4h | backend |
| HIGH | Clean Library API | 4h | api |
| HIGH | Response Hook | 6h | backend |
| MEDIUM | Remove Unnecessary | 2h | backend |
| MEDIUM | Simplify Config | 2h | backend |
| MEDIUM | Integration Tests | 4h | testing |
| LOW | Update Docs | 2h | setup |

**Total Effort:** ~4-5 days

**Recommended Order:**
1. SSE Parsing (unblocks HalluciGuard)
2. Feature Flags (enables minimal builds)
3. Response Hook + Clean API (together)
4. Remove Unnecessary + Simplify Config (cleanup)
5. Tests + Docs (polish)