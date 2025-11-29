//! Proxy layer architecture matching mitmproxy's design
//!
//! This module implements a layer-based proxy architecture using the sans-io pattern.
//! Layers represent protocol layers (TCP, TLS, HTTP, etc.) and are nested to handle
//! different protocol stacks.

pub mod commands;
pub mod context;
pub mod events;
pub mod layer;
pub mod layers;
pub mod server;
pub mod tunnel;

pub use commands::*;
pub use context::*;
pub use events::*;
pub use layer::*;
pub use server::*;