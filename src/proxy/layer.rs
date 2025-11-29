//! Base layer trait and implementation
//!
//! Layers interface with their child layer(s) by calling .handle_event(event),
//! which returns a generator (iterator) of commands.
//! Most layers do not implement .handle_event directly, but instead implement ._handle_event,
//! which is called by the default implementation of .handle_event.
//! The default implementation of .handle_event allows layers to emulate blocking code:
//! When ._handle_event yields a command that has its blocking attribute set to True,
//! .handle_event pauses the execution of ._handle_event and waits until it is called
//! with the corresponding CommandCompleted event.

use crate::proxy::{commands::Command, context::Context, events::{AnyEvent, CommandCompleted}};
use std::collections::VecDeque;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use crate::error::ProxyError;

/// Maximum size of individual log statements before they will be truncated.
const MAX_LOG_STATEMENT_SIZE: usize = 2048;

/// A generator that yields commands and ultimately returns a value.
/// This is similar to Python's generator pattern used in mitmproxy.
pub trait CommandGenerator<T> {
    fn next_command(&mut self) -> Option<Box<dyn Command>>;
    fn is_complete(&self) -> bool;
    fn get_result(self) -> Option<T>;
    fn handle_reply(&mut self, reply: CommandCompleted);
}

/// Simple command generator that just yields a list of commands
pub struct SimpleCommandGenerator {
    commands: VecDeque<Box<dyn Command>>,
    complete: bool,
}

impl SimpleCommandGenerator {
    pub fn new(commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            commands: commands.into(),
            complete: false,
        }
    }

    pub fn empty() -> Self {
        Self {
            commands: VecDeque::new(),
            complete: true,
        }
    }
}

impl CommandGenerator<()> for SimpleCommandGenerator {
    fn next_command(&mut self) -> Option<Box<dyn Command>> {
        if let Some(cmd) = self.commands.pop_front() {
            Some(cmd)
        } else {
            self.complete = true;
            None
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn get_result(self) -> Option<()> {
        if self.complete {
            Some(())
        } else {
            None
        }
    }

    fn handle_reply(&mut self, _reply: CommandCompleted) {
        // Simple generator doesn't handle replies
    }
}

/// Boolean command generator for HTTP/2 event handling
pub struct BooleanCommandGenerator {
    commands: VecDeque<Box<dyn Command>>,
    result: bool,
    complete: bool,
}

impl BooleanCommandGenerator {
    pub fn new(commands: Vec<Box<dyn Command>>, result: bool) -> Self {
        Self {
            commands: commands.into(),
            result,
            complete: false,
        }
    }

    pub fn with_result(result: bool) -> Self {
        Self {
            commands: VecDeque::new(),
            result,
            complete: true,
        }
    }
}

impl CommandGenerator<bool> for BooleanCommandGenerator {
    fn next_command(&mut self) -> Option<Box<dyn Command>> {
        if let Some(cmd) = self.commands.pop_front() {
            Some(cmd)
        } else {
            self.complete = true;
            None
        }
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn get_result(self) -> Option<bool> {
        if self.complete {
            Some(self.result)
        } else {
            None
        }
    }

    fn handle_reply(&mut self, _reply: CommandCompleted) {
        // Simple generator doesn't handle replies
    }
}

/// Generator that converts async operations to sync CommandGenerator pattern
/// This allows async methods to be converted to sync methods returning CommandGenerators
pub struct AsyncToSyncGenerator<T> {
    future: Option<Pin<Box<dyn Future<Output = Result<Vec<Box<dyn Command>>, ProxyError>> + Send>>>,
    commands: VecDeque<Box<dyn Command>>,
    result: Option<T>,
    complete: bool,
}

impl<T> AsyncToSyncGenerator<T> {
    pub fn new(future: Pin<Box<dyn Future<Output = Result<Vec<Box<dyn Command>>, ProxyError>> + Send>>) -> Self {
        Self {
            future: Some(future),
            commands: VecDeque::new(),
            result: None,
            complete: false,
        }
    }

    pub fn with_commands(commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            future: None,
            commands: commands.into(),
            result: None,
            complete: false,
        }
    }
}

impl<T: Default> CommandGenerator<T> for AsyncToSyncGenerator<T> {
    fn next_command(&mut self) -> Option<Box<dyn Command>> {
        if let Some(cmd) = self.commands.pop_front() {
            return Some(cmd);
        }

        if let Some(_future) = self.future.take() {
            // Convert async future to sync execution - in a real implementation,
            // this would use a runtime or be handled by the proxy server
            // For now, we'll return an error command indicating async conversion needed
            let error_cmd = Box::new(crate::proxy::commands::Log {
                message: "Async to sync conversion not yet implemented".to_string(),
                level: crate::proxy::commands::LogLevel::Error,
            }) as Box<dyn Command>;
            self.complete = true;
            return Some(error_cmd);
        }

        self.complete = true;
        None
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn get_result(self) -> Option<T> {
        if self.complete {
            self.result.or_else(|| Some(T::default()))
        } else {
            None
        }
    }

    fn handle_reply(&mut self, _reply: CommandCompleted) {
        // TODO: Handle async command completion
    }
}

/// Generator for processing HTTP/2 events
pub struct H2EventGenerator {
    events: VecDeque<crate::proxy::layers::http::H2Event>,
    commands: VecDeque<Box<dyn Command>>,
    complete: bool,
}

impl H2EventGenerator {
    pub fn new(events: Vec<crate::proxy::layers::http::H2Event>) -> Self {
        Self {
            events: events.into(),
            commands: VecDeque::new(),
            complete: false,
        }
    }

    pub fn with_commands(commands: Vec<Box<dyn Command>>) -> Self {
        Self {
            events: VecDeque::new(),
            commands: commands.into(),
            complete: false,
        }
    }
}

impl CommandGenerator<()> for H2EventGenerator {
    fn next_command(&mut self) -> Option<Box<dyn Command>> {
        if let Some(cmd) = self.commands.pop_front() {
            return Some(cmd);
        }

        if let Some(_event) = self.events.pop_front() {
            // Process the H2 event and generate commands
            // This would be handled by the HTTP/2 layer's event processing
            // For now, just mark as complete
        }

        self.complete = true;
        None
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn get_result(self) -> Option<()> {
        if self.complete {
            Some(())
        } else {
            None
        }
    }

    fn handle_reply(&mut self, _reply: CommandCompleted) {
        // Handle H2 event processing replies
    }
}

/// State of a layer that's paused because it is waiting for a command reply.
#[derive(Debug)]
pub struct Paused {
    pub command: Box<dyn Command>,
    pub generator: Box<dyn Any + Send + Sync>, // Store the generator state
}

/// Base trait for all protocol layers.
///
/// Layers interface with their child layer(s) by calling .handle_event(event),
/// which returns a generator of commands.
pub trait Layer: Send + Sync + std::fmt::Debug {
    /// Handle an event and return a command generator.
    /// This is the main entry point that handles the blocking semantics.
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>>;

    /// Internal event handler that layers should implement.
    /// This can yield blocking commands and will be paused/resumed automatically.
    fn _handle_event(&mut self, _event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // Default implementation just returns empty generator
        Box::new(SimpleCommandGenerator::empty())
    }

    /// Get the layer name for debugging
    fn layer_name(&self) -> &'static str;

    /// Get debug prefix for log messages
    fn debug_prefix(&self) -> Option<&str> {
        None
    }
}

/// Base layer implementation with common functionality
#[derive(Debug)]
pub struct BaseLayer {
    pub context: Context,
    pub paused: Option<Paused>,
    pub paused_event_queue: VecDeque<AnyEvent>,
    pub debug: Option<String>,
}

impl BaseLayer {
    pub fn new(context: Context) -> Self {
        let debug = if context.options.proxy_debug {
            Some("  ".repeat(context.layers.len()))
        } else {
            None
        };

        Self {
            context,
            paused: None,
            paused_event_queue: VecDeque::new(),
            debug,
        }
    }

    /// Check if the layer is currently paused
    pub fn is_paused(&self) -> bool {
        self.paused.is_some()
    }

    /// Pause execution with a blocking command
    pub fn pause_with_command(&mut self, command: Box<dyn Command>, generator: Box<dyn Any + Send + Sync>) {
        self.paused = Some(Paused { command, generator });
    }

    /// Resume execution after a command completes
    pub fn resume(&mut self) -> Option<(Box<dyn Any + Send + Sync>, VecDeque<AnyEvent>)> {
        if let Some(paused) = self.paused.take() {
            let events = std::mem::take(&mut self.paused_event_queue);
            Some((paused.generator, events))
        } else {
            None
        }
    }

    /// Queue an event while paused
    pub fn queue_event(&mut self, event: AnyEvent) {
        if self.is_paused() {
            self.paused_event_queue.push_back(event);
        }
    }

    /// Create a debug log command
    pub fn debug_log(&self, message: &str) -> Option<Box<dyn Command>> {
        if let Some(prefix) = &self.debug {
            let truncated = if message.len() > MAX_LOG_STATEMENT_SIZE {
                format!("{}...", &message[..MAX_LOG_STATEMENT_SIZE])
            } else {
                message.to_string()
            };

            Some(Box::new(crate::proxy::commands::Log {
                message: format!("{}{}", prefix, truncated),
                level: crate::proxy::commands::LogLevel::Debug,
            }))
        } else {
            None
        }
    }
}

/// NextLayer is used to determine which layer should handle a connection
#[derive(Debug)]
pub struct NextLayer {
    base: BaseLayer,
    child_layer: Option<Box<dyn Layer>>,
    buffered_events: Vec<AnyEvent>,
}

impl NextLayer {
    pub fn new(context: Context) -> Self {
        Self {
            base: BaseLayer::new(context),
            child_layer: None,
            buffered_events: Vec::new(),
        }
    }

    pub fn set_child_layer(&mut self, layer: Box<dyn Layer>) {
        self.child_layer = Some(layer);
    }

    /// Process all buffered events through the child layer
    fn process_buffered_events(&mut self) -> Box<dyn CommandGenerator<()>> {
        if let Some(ref mut child) = self.child_layer {
            let mut all_commands = Vec::new();

            for event in self.buffered_events.drain(..) {
                let mut generator = child.handle_event(event);
                while let Some(cmd) = generator.next_command() {
                    all_commands.push(cmd);
                }
            }

            Box::new(SimpleCommandGenerator::new(all_commands))
        } else {
            Box::new(SimpleCommandGenerator::empty())
        }
    }
}

impl Layer for NextLayer {
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        if let Some(ref mut child) = self.child_layer {
            child.handle_event(event)
        } else {
            // Buffer the event until we have a child layer
            self.buffered_events.push(event);

            // TODO: Implement proper layer selection logic based on the event type
            // For now, default to TCP layer
            let tcp_layer = Box::new(crate::proxy::layers::tcp::TcpLayer::new(self.base.context.clone()));
            self.set_child_layer(tcp_layer);

            self.process_buffered_events()
        }
    }

    fn layer_name(&self) -> &'static str {
        "NextLayer"
    }

    fn debug_prefix(&self) -> Option<&str> {
        self.base.debug.as_deref()
    }
}