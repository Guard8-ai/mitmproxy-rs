# mitmproxy-rs

A Rust HTTPS interception library with mitmproxy-compatible API.

## Overview

**mitmproxy-rs** is a high-performance Rust library for HTTPS traffic interception. It provides the core proxy infrastructure that can be embedded into other applications.

**Primary Use Case:** [HalluciGuard](https://github.com/Guard8-ai/HalluciGuard) uses this library for real-time AI API monitoring and hallucination detection.

## Status

🚧 **Work in Progress** - Core functionality is implemented but compilation fixes are needed.

See [COMPILATION_FIXES.md](COMPILATION_FIXES.md) for current blockers.

## Using as a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
mitmproxy-rs = { git = "https://github.com/Guard8-ai/mitmproxy-rs" }
```

### Basic Usage

```rust
use mitmproxy_rs::{ProxyBuilder, ResponseInterceptor, HTTPFlow};

struct MyInterceptor;

impl ResponseInterceptor for MyInterceptor {
    fn on_response(&self, flow: &mut HTTPFlow) {
        // Inspect/modify response
        if let Some(body) = &flow.response.content {
            println!("Response: {} bytes", body.len());
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy = ProxyBuilder::new()
        .listen_port(8080)
        .interceptor(MyInterceptor)
        .build()?;

    proxy.run().await
}
```

### SSE Stream Parsing (for AI APIs)

```rust
use mitmproxy_rs::sse::SseParser;

// Parse Claude/OpenAI streaming responses
let parser = SseParser::new();
for event in parser.parse(response_body) {
    if event.event_type == "content_block_delta" {
        let text = event.get_text();
        // Process streaming text
    }
}
```

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `http-proxy` | HTTP/1.1 proxy support | ✅ |
| `tls-intercept` | HTTPS interception with CA certs | ✅ |
| `http2` | HTTP/2 protocol support | ❌ |
| `rest-api` | mitmproxy-compatible REST API | ❌ |
| `sse-parsing` | Server-Sent Events parsing | ❌ |

### Minimal Build (for HalluciGuard)

```toml
[dependencies]
mitmproxy-rs = {
    git = "https://github.com/Guard8-ai/mitmproxy-rs",
    default-features = false,
    features = ["http-proxy", "tls-intercept", "sse-parsing"]
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Your Application                                               │
│  (HalluciGuard, cloud-mitmproxy, custom tools)                  │
│  • Pattern detection, alerting, business logic                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ imports / REST API
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  mitmproxy-rs                                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  REST API   │  │   Health    │  │   Management API        │  │
│  │  /flows     │  │   /health   │  │   /api/proxy/start|stop │  │
│  │  /updates   │  │   /ready    │  │   /api/config           │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  Proxy Core                                               │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────┐   │  │
│  │  │  HTTP   │  │  HTTPS  │  │  HTTP/2 │  │  WebSocket  │   │  │
│  │  │  Layer  │  │   TLS   │  │  Layer  │  │    Layer    │   │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────────┘   │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │  Certificate  │  │  SSE Stream   │  │  Response Hooks     │  │
│  │  Authority    │  │  Parser       │  │  (Interceptors)     │  │
│  └───────────────┘  └───────────────┘  └─────────────────────┘  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Deployment Options                                             │
│  • Library embed    • Docker container    • Standalone binary   │
│  • ECS/Fargate      • Kubernetes          • Local development   │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### Proxy Server

```rust
use mitmproxy_rs::ProxyServer;

let server = ProxyServer::new(config);
server.run().await?;
```

### Certificate Authority

```rust
use mitmproxy_rs::CertificateAuthority;

let ca = CertificateAuthority::new("~/.mitmproxy-rs/certs")?;
let cert = ca.get_cert_for_host("api.anthropic.com")?;
```

### Flow Data

```rust
use mitmproxy_rs::{HTTPFlow, HTTPRequest, HTTPResponse};

// Access request details
let method = &flow.request.method;
let host = &flow.request.host;
let headers = &flow.request.headers;

// Access response details
let status = flow.response.status_code;
let body = &flow.response.content;
```

## REST API (Optional)

Enable with `features = ["rest-api"]` for mitmproxy-compatible endpoints:

- `GET /flows` - List captured flows
- `GET /flows/{id}` - Get specific flow
- `PUT /flows/{id}` - Modify flow
- `WS /updates` - Real-time flow updates

## Development

### Building

```bash
cargo build --release

# Minimal build (no REST API)
cargo build --release --no-default-features --features "http-proxy,tls-intercept"
```

### Testing

```bash
cargo test
cargo test --features "rest-api,sse-parsing"
```

### Project Structure

```
src/
├── lib.rs              # Library exports
├── proxy/              # Proxy implementation
│   ├── mod.rs
│   └── layers/         # Protocol layers
│       ├── http.rs     # HTTP/1.1 & HTTP/2
│       └── tls.rs      # TLS interception
├── flow.rs             # Flow data structures
├── certs.rs            # Certificate management
├── sse.rs              # SSE parsing (optional)
└── api/                # REST API (optional)
```

## Related Projects

- [HalluciGuard](https://github.com/Guard8-ai/HalluciGuard) - AI hallucination detection using this library
- [mitmproxy](https://mitmproxy.org/) - Original Python implementation

## License

MIT License - see [LICENSE](LICENSE) file.
