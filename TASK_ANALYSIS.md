# mitmproxy-rs: HalluciGuard Integration Task Analysis

## Project Context

**Goal:** Make mitmproxy-rs a standalone, reusable HTTPS interception library that HalluciGuard imports as a dependency.

**Architecture:**
```
mitmproxy-rs (Generic Library)
    │
    └── HalluciGuard imports → uses for HTTPS interception + SSE parsing
```

---

## Task Breakdown

### Fix #1: Add SSE Stream Parsing Module
**Priority:** CRITICAL
**Effort:** 1 day
**Area:** backend

Implement Server-Sent Events parsing for LLM API responses (Claude, OpenAI, etc.).

#### Context
Currently HTTP responses are captured as raw bodies. LLM APIs use SSE format:
```
data: {"type": "content_block_delta", "delta": {"text": "Hello"}}

data: {"type": "message_stop"}

data: [DONE]
```

#### Implementation
Create `src/sse.rs`:
- Parse line-based SSE format
- Extract `data:`, `event:`, `id:`, `retry:` fields
- Handle multi-line data values
- Buffer incomplete events across chunks
- Return iterator of parsed events

#### Acceptance Criteria
- [ ] Parse standard SSE format (data, event, id, retry fields)
- [ ] Handle chunked/streaming responses
- [ ] Extract JSON payloads from data fields
- [ ] Unit tests with real Claude API response samples

---

### Fix #2: Create Library Feature Flags
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

### Fix #3: Export Clean Library API
**Priority:** HIGH
**Effort:** 4 hours
**Area:** api
**Dependencies:** [backend-001, backend-002]

Define stable public API in lib.rs for HalluciGuard consumption.

#### Current State
lib.rs exists but exports everything. Need curated public API.

#### Implementation
```rust
// src/lib.rs - Clean public API
pub mod proxy;
pub mod flow;
pub mod certs;
pub mod sse;

pub use flow::{Flow, HTTPFlow, HTTPRequest, HTTPResponse};
pub use proxy::ProxyServer;
pub use certs::CertificateAuthority;
pub use sse::SseParser;

pub struct MitmproxyBuilder { ... }
```

#### Acceptance Criteria
- [ ] Public API is minimal and documented
- [ ] Internal modules are private
- [ ] Builder pattern for proxy configuration
- [ ] Example in README showing HalluciGuard-style usage

---

### Fix #4: Add Response Interception Hook
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

### Fix #5: Remove Unnecessary Components
**Priority:** MEDIUM
**Effort:** 2 hours
**Area:** backend

Remove or gate components not needed for library use.

#### Components to Remove/Gate
Behind `addon-system` feature (disabled by default):
- Command execution framework (src/api/commands)
- Addon loading system

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

### Fix #6: Simplify Configuration
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

### Fix #7: Add Integration Tests
**Priority:** MEDIUM
**Effort:** 4 hours
**Area:** testing
**Dependencies:** [backend-001, api-001]

Create integration tests simulating HalluciGuard usage.

#### Test Scenarios
1. Intercept HTTPS request to api.anthropic.com
2. Parse SSE streaming response
3. Extract text from content_block_delta events
4. Verify pattern detection hook called

#### Acceptance Criteria
- [ ] Test with mock Anthropic API responses
- [ ] Test SSE parsing with real response samples
- [ ] Test interceptor hook invocation
- [ ] CI passes with integration tests

---

### Fix #8: Update Documentation
**Priority:** LOW
**Effort:** 2 hours
**Area:** setup
**Dependencies:** [backend-002, api-001]

Update README for library usage focus.

#### Changes
- Add "Using as a Library" section
- Show HalluciGuard-style integration example
- Document feature flags
- Remove/minimize full mitmproxy clone focus
- Add API documentation

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
