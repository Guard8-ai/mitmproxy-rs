//! TCP layer implementation
//! This mirrors the Python TCP layer in mitmproxy/proxy/layers/tcp.py

use crate::proxy::{
    commands::{Command, Log, LogLevel, SendData},
    context::Context,
    events::{ConnectionClosed, DataReceived, Event, Start, AnyEvent},
    layer::{BaseLayer, CommandGenerator, Layer, SimpleCommandGenerator},
};

/// TCP layer that handles basic TCP connection management
#[derive(Debug)]
pub struct TcpLayer {
    base: BaseLayer,
}

impl TcpLayer {
    pub fn new(context: Context) -> Self {
        let mut context = context;
        context.add_layer("TCP".to_string());
        let base = BaseLayer::new(context);

        Self { base }
    }

    fn handle_start(&mut self) -> Box<dyn CommandGenerator<()>> {
        let mut commands = Vec::new();

        if let Some(log_cmd) = self.base.debug_log("TCP layer started") {
            commands.push(log_cmd);
        }

        // TCP layer is ready to receive data
        Box::new(SimpleCommandGenerator::new(commands))
    }

    fn handle_data_received(&mut self, data: Vec<u8>) -> Box<dyn CommandGenerator<()>> {
        let mut commands = Vec::new();

        if let Some(log_cmd) = self.base.debug_log(&format!("TCP received {} bytes", data.len())) {
            commands.push(log_cmd);
        }

        // In a real implementation, TCP layer would typically forward data to upper layers
        // For now, we'll just echo it back (for testing purposes)
        if let Some(server) = &self.base.context.server {
            commands.push(Box::new(SendData {
                connection: server.connection.clone(),
                data,
            }));
        }

        Box::new(SimpleCommandGenerator::new(commands))
    }

    fn handle_connection_closed(&mut self) -> Box<dyn CommandGenerator<()>> {
        let mut commands = Vec::new();

        if let Some(log_cmd) = self.base.debug_log("TCP connection closed") {
            commands.push(log_cmd);
        }

        Box::new(SimpleCommandGenerator::new(commands))
    }
}

impl Layer for TcpLayer {
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // Check if paused and queue events
        if self.base.is_paused() {
            self.base.queue_event(event);
            return Box::new(SimpleCommandGenerator::empty());
        }

        let event_name = event.event_name();

        if let Some(_log_cmd) = self.base.debug_log(&format!(">> {}", event_name)) {
            // Note: In production, we might want to be more selective about debug logging
        }

        match event {
            AnyEvent::Start(_) => self.handle_start(),
            AnyEvent::DataReceived(data_event) => {
                self.handle_data_received(data_event.data)
            }
            AnyEvent::ConnectionClosed(_) => self.handle_connection_closed(),
            _ => {
                // Unknown event, log it
                let mut commands = Vec::new();
                if let Some(log_cmd) = self.base.debug_log(&format!("Unknown event: {}", event_name)) {
                    commands.push(log_cmd);
                }
                Box::new(SimpleCommandGenerator::new(commands))
            }
        }
    }

    fn layer_name(&self) -> &'static str {
        "TCPLayer"
    }

    fn debug_prefix(&self) -> Option<&str> {
        self.base.debug.as_deref()
    }
}