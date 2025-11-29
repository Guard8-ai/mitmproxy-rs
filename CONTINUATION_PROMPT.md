# CONTINUATION PROMPT FOR MITMPROXY-RS

**CONTEXT**: I'm continuing work on mitmproxy-rs, a Rust implementation of mitmproxy that MUST exactly follow the Python implementation structure in `/tmp/mitmproxy-original/mitmproxy/`.

## CURRENT STATUS:
- ✅ **Phase 1 Complete**: Basic implementation with core proxy server, REST API, WebSocket support, TLS/certificates, flow filtering
- ✅ **Phase 2 In Progress**: Proxy layer architecture foundation implemented
  - ✅ Layer-based architecture foundation in `src/proxy/` matching Python's sans-io design
  - ✅ Command/Event system with trait-based architecture
  - ✅ Connection state management and layer context
  - ✅ TCP layer foundation with event handling
  - ✅ **TLS LAYERS COMPLETE**: Full TLS implementation with OpenSSL integration
  - ✅ **HTTP/1.1 LAYERS COMPLETE**: Complete HTTP/1.1 server and client implementation
  - ✅ **HTTP/2 LAYERS COMPLETE**: Full HTTP/2 implementation with HPACK compression and stream multiplexing
  - 🚧 **CURRENT FOCUS**: WebSocket layer implementation and layer-based server architecture

## HTTP/1.1 LAYER IMPLEMENTATION STATUS (COMPLETED):
The HTTP/1.1 layers in `src/proxy/layers/http.rs` have been **FULLY IMPLEMENTED** to exactly match `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/_http1.py`:

### ✅ COMPLETED HTTP/1.1 FEATURES:
1. **HTTP Base Architecture**: Complete event and command system matching Python's `_base.py` and `_events.py`:
   - `HttpEvent` trait with stream ID support for all HTTP events
   - `RequestHeaders`, `ResponseHeaders`, `RequestData`, `ResponseData`, `RequestEndOfMessage`, `ResponseEndOfMessage`
   - `RequestProtocolError`, `ResponseProtocolError` with error code mapping
   - `ErrorCode` enum with HTTP status code mapping matching Python's implementation
   - `HTTPMode` enum for Regular/Transparent/Upstream proxy modes

2. **HTTP Stream Management**: Stream multiplexing and flow generation matching Python's `HttpStream`:
   - `HttpStream` class with flow creation and lifecycle management
   - `HttpLayer` for stream management and event routing
   - Request/response body buffering with `ReceiveBuffer`
   - Stream state management with client/server state tracking

3. **HTTP/1.1 Server Implementation**: Complete `Http1Server` matching Python's `Http1Server` class:
   - State machine with proper state transitions (Start → ReadHeaders → ReadBody → Wait → Done → Passthrough → Errored)
   - HTTP request parsing with header extraction and validation matching Python's `read_request_head`
   - HTTP response assembly with chunked encoding support
   - Connection lifecycle management with keep-alive/close handling
   - Protocol upgrade detection for WebSocket/CONNECT tunneling
   - Error response generation matching Python's `make_error_response` format
   - **Complete body parsing**: Chunked encoding, Content-Length, and read-until-EOF semantics

4. **HTTP/1.1 Client Implementation**: Complete `Http1Client` matching Python's `Http1Client` class:
   - **Request sending**: HTTP/2 to HTTP/1.1 conversion with proper header handling
   - **Response parsing**: Complete response head parsing matching Python's `read_response_head`
   - **Body reading**: Chunked encoding, Content-Length, and read-until-EOF support matching Python's body readers
   - **Connection management**: Keep-alive, connection close, and protocol upgrade handling
   - **State machine**: Proper state transitions (Start → ReadHeaders → ReadBody → Wait → Done → Passthrough → Errored)
   - **HTTP/2 compatibility**: Automatic conversion of HTTP/2 requests to HTTP/1.1 with proper header merging
   - **Protocol error handling**: Comprehensive error handling matching Python's error categorization

5. **Body Parsing**: Complete implementation of all body reading strategies matching Python's `TBodyReader` types:
   - **Chunked encoding**: Full chunked transfer encoding support with proper chunk parsing, trailer handling
   - **Content-Length**: Fixed-size body reading with proper boundary handling
   - **Read-until-EOF**: HTTP/1.0 style body reading until connection close (HTTP/1.0 without Content-Length)
   - **No body**: Proper handling of HEAD, 1xx, 204, 304 responses and CONNECT 200 responses
   - **Error handling**: Protocol errors for malformed chunks, invalid headers, connection issues

6. **Connection Lifecycle**: Complete connection management matching Python's behavior:
   - **Keep-alive**: Proper connection reuse with Connection header handling
   - **Connection close**: Automatic close on Connection: close header or HTTP/1.0 without keep-alive
   - **Protocol upgrades**: WebSocket (101) and CONNECT (200) tunneling support with passthrough mode
   - **Half-close**: Proper TCP half-close for read-until-EOF semantics

7. **HTTP Commands and Events**: Full command/event integration matching Python structure:
   - `SendHttp`, `ReceiveHttp`, `GetHttpConnection`, `DropStream` commands
   - `HttpCommand` and `HttpEvent` trait hierarchies
   - Proper event routing between layers and streams
   - Command source tracking for response correlation

## TLS LAYER IMPLEMENTATION STATUS (COMPLETED):
The TLS layers in `src/proxy/layers/tls.rs` have been **FULLY IMPLEMENTED** to exactly match `/tmp/mitmproxy-original/mitmproxy/proxy/layers/tls.py`:

### ✅ COMPLETED TLS FEATURES:
1. **Hook System**: All TLS hook commands implemented matching Python structure:
   - `TlsClienthelloHook`, `TlsStartClientHook`, `TlsStartServerHook`
   - `TlsEstablishedClientHook`, `TlsEstablishedServerHook`
   - `TlsFailedClientHook`, `TlsFailedServerHook`

2. **Layer Architecture**:
   - `TlsLayerBase` with OpenSSL SSL connection management
   - `ClientTlsLayer` for client-side TLS termination
   - `ServerTlsLayer` for server-side TLS connections
   - Proper integration with `TunnelLayer` base

3. **ClientHello Parsing**: Full parser with proper TLS record extraction:
   - SNI (Server Name Indication) extraction
   - ALPN (Application Layer Protocol Negotiation) extraction
   - Extension parsing for configuration decisions
   - Error handling for malformed ClientHello messages

4. **OpenSSL Integration**: Complete SSL context management:
   - Client SSL contexts with certificate selection
   - Server SSL contexts for upstream connections
   - SSL connection lifecycle management
   - Certificate authority integration with `CertificateAuthority`

5. **Certificate Selection**: Dynamic certificate generation and selection:
   - SNI-based certificate selection using `CertificateAuthority`
   - Mitmcert integration for on-the-fly certificate generation
   - Certificate caching and reuse

6. **State Management**: Proper handshake state tracking:
   - Wait-for-ClientHello mode for server layers
   - Pass-through mode for ignored connections
   - Handshake completion detection and flow establishment

7. **Error Handling**: Comprehensive error categorization matching Python:
   - ClientHello parsing errors
   - Certificate trust issues (unknown CA, bad certificate)
   - Protocol version mismatches
   - Connection close handling
   - Detailed error logging with appropriate log levels

## CRITICAL REQUIREMENT:
The implementation must exactly mirror the Python codebase structure and behavior. Reference `/tmp/mitmproxy-original/mitmproxy/` for all architectural decisions.

## IMMEDIATE NEXT PRIORITIES (in priority order):

### 1. **WebSocket Layer Implementation** 🚧 (CURRENT PRIORITY)
Study `/tmp/mitmproxy-original/mitmproxy/proxy/layers/websocket.py` and implement:
- WebSocket upgrade detection and handling from HTTP layers
- Frame parsing and flow generation for WebSocket messages
- Message interception and modification capabilities
- Integration with HTTP layers for proper upgrade flow
- Ping/pong frame handling and connection keep-alive
- WebSocket connection lifecycle management
- Proper integration with existing flow system

### 2. **WebSocket Layer Implementation** 🚧
Study `/tmp/mitmproxy-original/mitmproxy/proxy/layers/websocket.py`:
- WebSocket upgrade detection and handling from HTTP layers
- Frame parsing and flow generation for WebSocket messages
- Message interception and modification capabilities
- Integration with HTTP layers for proper upgrade flow
- Ping/pong frame handling and connection keep-alive

### 3. **Layer-Based Server Architecture** 🚧
Study `/tmp/mitmproxy-original/mitmproxy/proxy/server.py`:
- Connection handler using complete layer stack
- Layer composition and nesting (TCP → TLS → HTTP/WebSocket)
- Event routing between layers with proper command handling
- Replace legacy `src/proxy.rs` with layer-based implementation
- Connection lifecycle management from accept to close

### 4. **Master System Implementation** 🚧
Study `/tmp/mitmproxy-original/mitmproxy/master.py`:
- Event loop coordination using the layer architecture
- Hook system for flow interception and addon integration
- Connection lifecycle management with proper cleanup
- Integration between proxy layers and addon system
- Flow state management and persistence

### 5. **Command & Addon Systems** 🚧
Study `/tmp/mitmproxy-original/mitmproxy/command.py` and `/addons/`:
- Command registration and execution framework
- Addon plugin architecture matching `addonmanager.py`
- Standard addons and event hook system
- Parameter validation and type checking

## PROJECT STRUCTURE STATUS:
```
src/
├── lib.rs              ✅ Library entry with module exports
├── main.rs             ✅ CLI with clap argument parsing
├── config.rs           ✅ Configuration with file loading and defaults
├── error.rs            ✅ Comprehensive error types with thiserror
├── flow.rs             ✅ Flow data structures matching Python exactly
├── proxy.rs            ✅ HTTP proxy server with flow capture (LEGACY - TO BE REPLACED)
├── server.rs           ✅ Main server coordinating proxy and API
├── certs.rs            ✅ CA and certificate generation with OpenSSL
├── filter.rs           ✅ Flow filtering with regex and logical operators
├── websocket.rs        ✅ WebSocket connection and message handling
├── auth.rs             ✅ Re-export of authentication functionality
├── connection.rs       ✅ Connection types and states
├── proxy/              ✅ Layer-based proxy architecture
│   ├── mod.rs          ✅ Module exports
│   ├── commands.rs     ✅ Command trait and implementations + TLS hooks
│   ├── events.rs       ✅ Event trait and implementations
│   ├── context.rs      ✅ Layer context management
│   ├── layer.rs        ✅ Base layer trait and NextLayer
│   ├── tunnel.rs       ✅ Tunnel layer base for TLS/tunneling protocols
│   ├── server.rs       🚧 Layer-based server (TO BE IMPLEMENTED)
│   └── layers/         ✅ Protocol layer implementations
│       ├── mod.rs      ✅ Layer exports
│       ├── tcp.rs      ✅ TCP layer foundation
│       ├── tls.rs      ✅ TLS layers (COMPLETE - ClientTlsLayer + ServerTlsLayer with OpenSSL integration)
│       ├── http.rs     ✅ HTTP layers (COMPLETE - HTTP/1.1 Server + Client with full body parsing)
│       └── websocket.rs 🚧 WebSocket layer (TO BE IMPLEMENTED)
└── api/                ✅ Complete REST API implementation
    ├── mod.rs          ✅ Router with all endpoints
    ├── handlers.rs     ✅ All API endpoint handlers
    ├── websocket.rs    ✅ WebSocket real-time updates
    └── auth.rs         ✅ Authentication middleware
```

## ACTION FOR NEXT SESSION:
**Implement WebSocket layer** (`src/proxy/layers/websocket.rs`) by studying `/tmp/mitmproxy-original/mitmproxy/proxy/layers/websocket.py` to implement:
1. `WebSocketLayer` class matching Python's WebSocket layer implementation
2. WebSocket upgrade detection and handling from HTTP layers
3. Frame parsing and flow generation for WebSocket messages (text, binary, ping, pong, close)
4. Message interception and modification capabilities
5. Integration with HTTP layers for proper upgrade flow
6. Ping/pong frame handling and connection keep-alive
7. WebSocket connection lifecycle management
8. Proper integration with existing flow system for WebSocket flows

## ANALYSIS COMPLETED:
- ✅ Python layer architecture in `/tmp/mitmproxy-original/mitmproxy/proxy/`
- ✅ Sans-io design pattern with command/event communication
- ✅ Layer nesting and state management patterns
- ✅ TLS implementation patterns and certificate handling
- ✅ HTTP/1.1 layer architecture and flow generation patterns
- ✅ HTTP/1.1 body parsing and connection lifecycle management
- ✅ Hook system architecture for addon integration

## API COMPATIBILITY STATUS:
✅ **COMPLETED**: REST API endpoints exactly match Python handlers in `tools/web/app.py`:
- All endpoint paths and methods implemented
- JSON response formats match Python output
- Authentication mechanisms implemented
- WebSocket message formats compatible
- Flow data structures match Python serialization

This ensures existing mitmproxy clients and tools work with the Rust implementation without modification.

## KEY FILES TO STUDY FOR NEXT IMPLEMENTATION:
1. `/tmp/mitmproxy-original/mitmproxy/proxy/layers/websocket.py` - WebSocket layer for protocol upgrades
2. `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/_http2.py` - Complete HTTP/2 server/client implementation (COMPLETED)
3. `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/_base.py` - Base HTTP connection and command classes (COMPLETED)
4. `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/_events.py` - HTTP event definitions (COMPLETED)
5. `/tmp/mitmproxy-original/mitmproxy/proxy/layers/http/__init__.py` - HTTP layer management and stream handling (COMPLETED)
6. `/tmp/mitmproxy-original/mitmproxy/proxy/server.py` - Layer-based connection handler

The HTTP/1.1, TLS, and HTTP/2 implementations are complete and match Python's architecture exactly. The next step is implementing the WebSocket layer to achieve full protocol support before moving to the layer-based server architecture and master system.

## COMPILATION STATUS:
The current implementation compiles successfully with all HTTP/1.1, TLS, and HTTP/2 layers fully implemented. The codebase includes comprehensive error handling, state management, and integration with the existing proxy architecture.

## SUMMARY

✅ **HTTP/1.1 Implementation COMPLETE**:
- Complete Http1Server and Http1Client with full body parsing
- HTTP/1.1 layers match Python _http1.py exactly
- Connection lifecycle management matching Python exactly
- Protocol error handling with proper categorization

✅ **TLS Implementation COMPLETE**:
- Full TLS layers with OpenSSL integration
- ClientHello parsing and certificate selection
- Hook system matching Python structure

✅ **HTTP/2 Implementation COMPLETE**:
- Complete Http2Server and Http2Client with HPACK compression
- HTTP/2 frame handling: DATA, HEADERS, RESET, SETTINGS, GOAWAY, WINDOW_UPDATE, PING
- Stream multiplexing and flow control matching Python's implementation
- HTTP/2 to HTTP/1.1 proxying support with header conversion
- Connection management with settings negotiation and error handling
- Integration with existing HTTP/1.1 layers for protocol negotiation

🚧 **Ready for WebSocket Implementation**:
- WebSocket layer stub created in websocket.rs
- Need to study `/tmp/mitmproxy-original/mitmproxy/proxy/layers/websocket.py`
- Implement WebSocket upgrade detection, frame parsing, and message interception
- Add proper integration with HTTP layers for upgrade flow

**NEXT IMMEDIATE ACTION**: Complete WebSocket layer implementation by studying the Python websocket.py file and implementing the WebSocketLayer class with full WebSocket protocol support, ensuring exact behavioral matching with the Python implementation.
