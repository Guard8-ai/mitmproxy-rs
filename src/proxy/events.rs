//! Events that are passed down to layers when IO actions occur

use crate::connection::Connection;
use crate::proxy::commands::Command;

// Macro to implement Event trait for simple event types
macro_rules! impl_event {
    ($type_name:ty, $event_name:expr) => {
        impl Event for $type_name {
            fn event_name(&self) -> &'static str {
                $event_name
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}

/// Base trait for all events
pub trait Event: std::fmt::Debug + Send + Sync + std::any::Any {
    fn event_name(&self) -> &'static str;

    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Every layer initially receives a start event
#[derive(Debug, Clone)]
pub struct Start;

/// All events involving connection IO
#[derive(Debug, Clone)]
pub struct ConnectionEvent {
    pub connection: Connection,
}

/// Remote has sent some data
#[derive(Debug, Clone)]
pub struct DataReceived {
    pub connection: Connection,
    pub data: Vec<u8>,
}

/// Remote has closed a connection
#[derive(Debug, Clone)]
pub struct ConnectionClosed {
    pub connection: Connection,
}

/// Emitted when a command has been completed
#[derive(Debug)]
pub struct CommandCompleted {
    pub command: Box<dyn Command>,
    pub reply: Option<Box<dyn std::any::Any + Send + Sync>>,
}

/// Request for connection to be opened has been completed
#[derive(Debug)]
pub struct OpenConnectionCompleted {
    pub command: Box<dyn Command>,
    pub error: Option<String>,
}

/// A wakeup event after a delay
#[derive(Debug, Clone)]
pub struct Wakeup {
    pub delay: f64,
}

/// Hook completed event
#[derive(Debug)]
pub struct HookCompleted {
    pub command: Box<dyn Command>,
}

/// WebSocket message injected event
#[derive(Debug, Clone)]
pub struct WebSocketMessageInjected {
    pub message: crate::flow::WebSocketMessage,
}

/// Type-erased event that can hold any event type
#[derive(Debug)]
pub enum AnyEvent {
    Start(Start),
    ConnectionEvent(ConnectionEvent),
    DataReceived(DataReceived),
    ConnectionClosed(ConnectionClosed),
    CommandCompleted(CommandCompleted),
    OpenConnectionCompleted(OpenConnectionCompleted),
    Wakeup(Wakeup),
    HookCompleted(HookCompleted),
    WebSocketMessageInjected(WebSocketMessageInjected),
}

impl Event for AnyEvent {
    fn event_name(&self) -> &'static str {
        match self {
            AnyEvent::Start(e) => e.event_name(),
            AnyEvent::ConnectionEvent(e) => e.event_name(),
            AnyEvent::DataReceived(e) => e.event_name(),
            AnyEvent::ConnectionClosed(e) => e.event_name(),
            AnyEvent::CommandCompleted(e) => e.event_name(),
            AnyEvent::OpenConnectionCompleted(e) => e.event_name(),
            AnyEvent::Wakeup(e) => e.event_name(),
            AnyEvent::HookCompleted(e) => e.event_name(),
            AnyEvent::WebSocketMessageInjected(e) => e.event_name(),
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

macro_rules! impl_from_event {
    ($variant:ident, $type:ty) => {
        impl From<$type> for AnyEvent {
            fn from(event: $type) -> Self {
                AnyEvent::$variant(event)
            }
        }
    };
}

impl_from_event!(Start, Start);
impl_from_event!(ConnectionEvent, ConnectionEvent);
impl_from_event!(DataReceived, DataReceived);
impl_from_event!(ConnectionClosed, ConnectionClosed);
impl_from_event!(CommandCompleted, CommandCompleted);
impl_from_event!(OpenConnectionCompleted, OpenConnectionCompleted);
impl_from_event!(Wakeup, Wakeup);
impl_from_event!(HookCompleted, HookCompleted);
impl_from_event!(WebSocketMessageInjected, WebSocketMessageInjected);

// Implement Event trait for all event types
impl_event!(Start, "Start");
impl_event!(ConnectionEvent, "ConnectionEvent");
impl_event!(DataReceived, "DataReceived");
impl_event!(ConnectionClosed, "ConnectionClosed");
impl_event!(CommandCompleted, "CommandCompleted");
impl_event!(OpenConnectionCompleted, "OpenConnectionCompleted");
impl_event!(Wakeup, "Wakeup");
impl_event!(HookCompleted, "HookCompleted");
impl_event!(WebSocketMessageInjected, "WebSocketMessageInjected");
