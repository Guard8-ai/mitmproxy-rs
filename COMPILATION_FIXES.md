# mitmproxy-rs: Compilation Fixes Required

## Current Status

**332 compilation errors** need to be resolved before the library is usable.

This document tracks the critical fixes needed to achieve a compiling codebase.

---

## Task Breakdown

### Fix #1: Remove h2::Reason Usage
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

### Fix #2: Convert Async Methods to Sync CommandGenerator
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

### Fix #3: Implement BufferedH2Connection
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

### Fix #4: Fix Layer Trait Implementations
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

### Fix #5: Fix Event Trait Methods
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
