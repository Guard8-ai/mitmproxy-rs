//! Tunnel layer base implementation matching mitmproxy's tunnel.py

use crate::connection::Connection;
use crate::proxy::{
    commands::Command,
    context::Context,
    events::{ConnectionClosed, DataReceived, Event, OpenConnectionCompleted, Start, AnyEvent},
    layer::{BaseLayer, CommandGenerator, Layer, SimpleCommandGenerator},
};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelState {
    Inactive,
    Establishing,
    Open,
    Closed,
}

/// Base tunnel layer that simplifies implementation of tunneling protocols such as TLS
#[derive(Debug)]
pub struct TunnelLayer {
    pub base: BaseLayer,
    pub tunnel_connection: Connection,
    pub conn: Connection,
    pub tunnel_state: TunnelState,
    pub command_to_reply_to: Option<Box<dyn Command>>,
    pub event_queue: VecDeque<AnyEvent>,
    pub child_layer: Option<Box<dyn Layer>>,
}

impl TunnelLayer {
    pub fn new(
        context: Context,
        tunnel_connection: Connection,
        conn: Connection,
    ) -> Self {
        Self {
            base: BaseLayer::new(context),
            tunnel_connection,
            conn,
            tunnel_state: TunnelState::Inactive,
            command_to_reply_to: None,
            event_queue: VecDeque::new(),
            child_layer: None,
        }
    }

    /// Start the handshake process
    pub fn start_handshake(&mut self) -> Vec<Box<dyn Command>> {
        self.receive_handshake_data(b"")
    }

    /// Handle handshake data reception
    pub fn receive_handshake_data(&mut self, _data: &[u8]) -> Vec<Box<dyn Command>> {
        // Default implementation - subclasses should override
        vec![]
    }

    /// Called when handshake encounters an error
    pub fn on_handshake_error(&mut self, _err: &str) -> Vec<Box<dyn Command>> {
        vec![Box::new(crate::proxy::commands::CloseConnection {
            connection: self.tunnel_connection.clone(),
        })]
    }

    /// Handle data reception after handshake is complete
    pub fn receive_data(&mut self, data: &[u8]) -> Vec<Box<dyn Command>> {
        if let Some(ref mut child) = self.child_layer {
            let mut generator = child.handle_event(AnyEvent::DataReceived(DataReceived {
                connection: self.conn.clone(),
                data: data.to_vec(),
            }));
            let mut commands = Vec::new();
            while let Some(cmd) = generator.next_command() {
                commands.push(cmd);
            }
            commands
        } else {
            vec![]
        }
    }

    /// Handle connection close
    pub fn receive_close(&mut self) -> Vec<Box<dyn Command>> {
        if let Some(ref mut child) = self.child_layer {
            let mut generator = child.handle_event(AnyEvent::ConnectionClosed(ConnectionClosed {
                connection: self.conn.clone(),
            }));
            let mut commands = Vec::new();
            while let Some(cmd) = generator.next_command() {
                commands.push(cmd);
            }
            commands
        } else {
            vec![]
        }
    }

    /// Send data through tunnel
    pub fn send_data(&mut self, data: &[u8]) -> Vec<Box<dyn Command>> {
        vec![Box::new(crate::proxy::commands::SendData {
            connection: self.tunnel_connection.clone(),
            data: data.to_vec(),
        })]
    }

    /// Send close command
    pub fn send_close(&mut self, command: Box<dyn Command>) -> Vec<Box<dyn Command>> {
        vec![command]
    }

    /// Forward event to child layer
    pub fn event_to_child_sync(&mut self, event: AnyEvent) -> Vec<Box<dyn Command>> {
        if self.tunnel_state == TunnelState::Establishing && self.command_to_reply_to.is_none() {
            self.event_queue.push_back(event);
            return vec![];
        }

        if let Some(ref mut child) = self.child_layer {
            let mut generator = child.handle_event(event);
            let mut commands = Vec::new();
            while let Some(cmd) = generator.next_command() {
                commands.push(cmd);
            }
            self.handle_child_commands(commands)
        } else {
            vec![]
        }
    }

    /// Handle commands from child layer
    pub fn handle_child_commands(&mut self, commands: Vec<Box<dyn Command>>) -> Vec<Box<dyn Command>> {
        let mut result = vec![];
        for command in commands {
            result.extend(self.handle_child_command(command));
        }
        result
    }

    /// Handle individual command from child layer
    pub fn handle_child_command(&mut self, command: Box<dyn Command>) -> Vec<Box<dyn Command>> {
        // Default implementation - forward commands
        vec![command]
    }

    /// Complete handshake process
    pub fn handshake_finished(&mut self, err: Option<&str>) -> Vec<Box<dyn Command>> {
        if err.is_some() {
            self.tunnel_state = TunnelState::Closed;
        } else {
            self.tunnel_state = TunnelState::Open;
        }

        if let Some(reply_cmd) = self.command_to_reply_to.take() {
            if let Some(ref mut child) = self.child_layer {
                let event = AnyEvent::OpenConnectionCompleted(OpenConnectionCompleted {
                    command: reply_cmd,
                    error: err.map(|s| s.to_string()),
                });
                let mut generator = child.handle_event(event);
                let mut commands = vec![];
                while let Some(cmd) = generator.next_command() {
                    commands.push(cmd);
                }
                commands
            } else {
                vec![]
            }
        } else {
            let mut commands = vec![];
            while let Some(_event) = self.event_queue.pop_front() {
                // TODO: Convert buffered events
                // commands.extend(self.event_to_child_sync(event));
            }
            commands
        }
    }
}

impl Layer for TunnelLayer {
    fn handle_event(&mut self, event: AnyEvent) -> Box<dyn CommandGenerator<()>> {
        // Check event type and dispatch accordingly
        match &event {
            AnyEvent::Start(start_event) => {
                self.tunnel_state = TunnelState::Establishing;
                let mut commands = self.start_handshake();
                commands.extend(self.event_to_child_sync(AnyEvent::Start(start_event.clone())));
                return Box::new(SimpleCommandGenerator::new(commands));
            }
            AnyEvent::DataReceived(data_event) => {
                if data_event.connection == self.tunnel_connection {
                    if self.tunnel_state == TunnelState::Establishing {
                        return Box::new(SimpleCommandGenerator::new(self.receive_handshake_data(&data_event.data)));
                    } else {
                        return Box::new(SimpleCommandGenerator::new(self.receive_data(&data_event.data)));
                    }
                }
            }
            AnyEvent::ConnectionClosed(close_event) => {
                if close_event.connection == self.tunnel_connection {
                    if self.tunnel_state == TunnelState::Open {
                        return Box::new(SimpleCommandGenerator::new(self.receive_close()));
                    } else if self.tunnel_state == TunnelState::Establishing {
                        let err = "connection closed";
                        let mut commands = self.on_handshake_error(err);
                        commands.extend(self.handshake_finished(Some(err)));
                        return Box::new(SimpleCommandGenerator::new(commands));
                    }
                    self.tunnel_state = TunnelState::Closed;
                }
            }
            _ => {}
        }

        let commands = self.event_to_child_sync(event);
        Box::new(SimpleCommandGenerator::new(commands))
    }

    fn layer_name(&self) -> &'static str {
        "TunnelLayer"
    }

}