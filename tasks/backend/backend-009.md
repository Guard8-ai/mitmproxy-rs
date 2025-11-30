---
id: backend-009
title: Fix Layer Trait Implementations
status: done
priority: high
tags:
- setup
- fix
dependencies:
- backend-007
assignee: developer
created: 2025-11-29T18:53:18.869004521Z
estimate: 3h
complexity: 4
area: backend
---

**Priority:** HIGH
**Effort:** 3 hours
**Area:** backend
**Dependencies:** [backend-007]

Ensure all layers correctly implement the Layer trait.

#### Problem
Layer implementations have signature mismatches with the trait definition.

#### Required Implementation
```rust
impl Layer for SomeLayer {
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // Convert AnyEvent to specific event type and delegate
    }

    fn layer_name(&self) -> &'static str {
        "SomeLayer"
    }
}
```

#### Layers to Fix
- TLS layers (ClientTlsLayer, ServerTlsLayer)
- HTTP layers (Http1Server, Http1Client, Http2Server, Http2Client)
- WebSocket layer

#### Acceptance Criteria
- [ ] All layers implement `Layer` trait
- [ ] `handle_event` signature matches trait
- [ ] `layer_name` returns correct identifier

---