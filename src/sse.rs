//! Server-Sent Events (SSE) parsing module for LLM API responses.
//!
//! This module provides parsing for SSE streams commonly used by LLM APIs
//! like Claude, OpenAI, and others. SSE is a W3C standard for server-push
//! over HTTP connections.
//!
//! # SSE Format
//! ```text
//! event: message_start
//! data: {"type": "message_start", "message": {...}}
//!
//! data: {"type": "content_block_delta", "delta": {"text": "Hello"}}
//!
//! data: [DONE]
//! ```
//!
//! # Example
//! ```rust
//! use mitmproxy_rs::sse::{SseParser, SseEvent};
//!
//! let mut parser = SseParser::new();
//! let events = parser.parse_chunk(b"data: {\"text\": \"Hello\"}\n\n");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Represents a single SSE event parsed from the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SseEvent {
    /// The event type (from `event:` field). Defaults to "message" if not specified.
    pub event_type: String,
    /// The data payload (from `data:` field). May contain multiple lines joined by newlines.
    pub data: String,
    /// The event ID (from `id:` field), if present.
    pub id: Option<String>,
    /// The retry timeout in milliseconds (from `retry:` field), if present.
    pub retry: Option<u64>,
}

impl SseEvent {
    /// Creates a new SSE event with the given data.
    pub fn new(data: String) -> Self {
        Self {
            event_type: "message".to_string(),
            data,
            id: None,
            retry: None,
        }
    }

    /// Creates a new SSE event with a specific event type.
    pub fn with_type(event_type: String, data: String) -> Self {
        Self {
            event_type,
            data,
            id: None,
            retry: None,
        }
    }

    /// Returns true if this is a "[DONE]" termination event (OpenAI format).
    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }

    /// Attempts to parse the data field as JSON.
    pub fn parse_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.data)
    }

    /// Attempts to parse the data field as a JSON value.
    pub fn as_json(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.data).ok()
    }
}

/// Parser state for processing SSE streams.
#[derive(Debug, Clone, Default)]
struct EventBuilder {
    event_type: Option<String>,
    data_lines: Vec<String>,
    id: Option<String>,
    retry: Option<u64>,
}

impl EventBuilder {
    fn new() -> Self {
        Self::default()
    }

    fn is_empty(&self) -> bool {
        self.event_type.is_none()
            && self.data_lines.is_empty()
            && self.id.is_none()
            && self.retry.is_none()
    }

    fn build(self) -> Option<SseEvent> {
        if self.data_lines.is_empty() {
            return None;
        }

        Some(SseEvent {
            event_type: self.event_type.unwrap_or_else(|| "message".to_string()),
            data: self.data_lines.join("\n"),
            id: self.id,
            retry: self.retry,
        })
    }

    fn reset(&mut self) {
        self.event_type = None;
        self.data_lines.clear();
        self.id = None;
        self.retry = None;
    }
}

/// SSE stream parser that handles chunked/streaming responses.
///
/// The parser maintains internal state to handle partial lines and
/// events that span multiple chunks.
#[derive(Debug, Clone)]
pub struct SseParser {
    /// Buffer for incomplete lines across chunks
    line_buffer: String,
    /// Current event being built
    current_event: EventBuilder,
    /// Last event ID for reconnection support
    last_event_id: Option<String>,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    /// Creates a new SSE parser.
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            current_event: EventBuilder::new(),
            last_event_id: None,
        }
    }

    /// Returns the last received event ID, useful for reconnection.
    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    /// Resets the parser state, clearing all buffers.
    pub fn reset(&mut self) {
        self.line_buffer.clear();
        self.current_event.reset();
    }

    /// Parses a chunk of SSE data and returns any complete events.
    ///
    /// This method handles:
    /// - Partial lines that span multiple chunks
    /// - Multiple events in a single chunk
    /// - Both `\n` and `\r\n` line endings
    pub fn parse_chunk(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let chunk_str = match std::str::from_utf8(chunk) {
            Ok(s) => s,
            Err(_) => return vec![], // Invalid UTF-8, skip chunk
        };

        self.parse_str(chunk_str)
    }

    /// Parses a string chunk of SSE data.
    pub fn parse_str(&mut self, chunk: &str) -> Vec<SseEvent> {
        let mut events = Vec::new();

        // Append to buffer and process complete lines
        self.line_buffer.push_str(chunk);

        // Process all complete lines
        while let Some(line_end) = self.find_line_end() {
            let line = self.line_buffer[..line_end].to_string();

            // Skip the line ending
            let skip = if self.line_buffer[line_end..].starts_with("\r\n") {
                2
            } else {
                1
            };
            self.line_buffer = self.line_buffer[line_end + skip..].to_string();

            // Process the line
            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }
        }

        events
    }

    /// Flushes any buffered event data and returns any pending event.
    ///
    /// Call this when the stream ends to get any final event that
    /// may not have been followed by a blank line.
    pub fn flush(&mut self) -> Option<SseEvent> {
        // Process any remaining data in the line buffer as a final line
        if !self.line_buffer.is_empty() {
            let remaining = std::mem::take(&mut self.line_buffer);
            self.process_line(&remaining);
        }

        let event = std::mem::take(&mut self.current_event);
        event.build()
    }

    /// Finds the end of the first complete line in the buffer.
    fn find_line_end(&self) -> Option<usize> {
        self.line_buffer.find('\n').map(|i| {
            // Handle \r\n by returning the position of \r if present
            if i > 0 && self.line_buffer.as_bytes().get(i - 1) == Some(&b'\r') {
                i - 1
            } else {
                i
            }
        })
    }

    /// Processes a single line and returns an event if one is complete.
    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        // Empty line indicates end of event
        if line.is_empty() {
            if !self.current_event.is_empty() {
                let event = std::mem::take(&mut self.current_event);
                if let Some(evt) = event.build() {
                    // Update last event ID if present
                    if evt.id.is_some() {
                        self.last_event_id = evt.id.clone();
                    }
                    return Some(evt);
                }
            }
            return None;
        }

        // Skip comments (lines starting with :)
        if line.starts_with(':') {
            return None;
        }

        // Parse field: value
        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let value = line[colon_pos + 1..].strip_prefix(' ').unwrap_or(&line[colon_pos + 1..]);
            (field, value)
        } else {
            // Field with no value
            (line, "")
        };

        match field {
            "event" => {
                self.current_event.event_type = Some(value.to_string());
            }
            "data" => {
                self.current_event.data_lines.push(value.to_string());
            }
            "id" => {
                // ID must not contain null characters
                if !value.contains('\0') {
                    self.current_event.id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(retry_ms) = value.parse::<u64>() {
                    self.current_event.retry = Some(retry_ms);
                }
            }
            _ => {
                // Ignore unknown fields per spec
            }
        }

        None
    }
}

/// Iterator adapter for parsing SSE events from a byte stream.
pub struct SseEventIterator<I> {
    parser: SseParser,
    source: I,
    pending_events: VecDeque<SseEvent>,
    finished: bool,
}

impl<I> SseEventIterator<I>
where
    I: Iterator<Item = Vec<u8>>,
{
    /// Creates a new SSE event iterator from a byte chunk iterator.
    pub fn new(source: I) -> Self {
        Self {
            parser: SseParser::new(),
            source,
            pending_events: VecDeque::new(),
            finished: false,
        }
    }
}

impl<I> Iterator for SseEventIterator<I>
where
    I: Iterator<Item = Vec<u8>>,
{
    type Item = SseEvent;

    fn next(&mut self) -> Option<Self::Item> {
        // Return pending events first
        if let Some(event) = self.pending_events.pop_front() {
            return Some(event);
        }

        if self.finished {
            return None;
        }

        // Read more chunks until we get an event or reach the end
        loop {
            match self.source.next() {
                Some(chunk) => {
                    let events = self.parser.parse_chunk(&chunk);
                    if !events.is_empty() {
                        let mut iter = events.into_iter();
                        let first = iter.next();
                        self.pending_events.extend(iter);
                        if first.is_some() {
                            return first;
                        }
                    }
                }
                None => {
                    self.finished = true;
                    // Flush any remaining event
                    return self.parser.flush();
                }
            }
        }
    }
}

/// Extension trait to create SSE event iterators from byte streams.
pub trait SseStreamExt: Iterator<Item = Vec<u8>> + Sized {
    /// Converts this byte chunk iterator into an SSE event iterator.
    fn sse_events(self) -> SseEventIterator<Self> {
        SseEventIterator::new(self)
    }
}

impl<I: Iterator<Item = Vec<u8>>> SseStreamExt for I {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_data_event() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: hello world\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "message");
        assert_eq!(events[0].data, "hello world");
    }

    #[test]
    fn test_event_with_type() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("event: custom\ndata: payload\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "custom");
        assert_eq!(events[0].data, "payload");
    }

    #[test]
    fn test_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: line1\ndata: line2\ndata: line3\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2\nline3");
    }

    #[test]
    fn test_event_with_id() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("id: 42\ndata: test\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some("42".to_string()));
        assert_eq!(parser.last_event_id(), Some("42"));
    }

    #[test]
    fn test_event_with_retry() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("retry: 5000\ndata: reconnect\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn test_skip_comments() {
        let mut parser = SseParser::new();
        let events = parser.parse_str(": this is a comment\ndata: actual data\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "actual data");
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: first\n\ndata: second\n\n");

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_chunked_input() {
        let mut parser = SseParser::new();

        // First chunk - incomplete
        let events1 = parser.parse_str("data: hel");
        assert!(events1.is_empty());

        // Second chunk - completes line but no event end
        let events2 = parser.parse_str("lo wor");
        assert!(events2.is_empty());

        // Third chunk - completes the event
        let events3 = parser.parse_str("ld\n\n");
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].data, "hello world");
    }

    #[test]
    fn test_crlf_line_endings() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: test\r\n\r\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "test");
    }

    #[test]
    fn test_empty_data_field() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data:\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "");
    }

    #[test]
    fn test_data_with_colon() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: key: value\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "key: value");
    }

    #[test]
    fn test_flush_incomplete_event() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data: incomplete");
        assert!(events.is_empty());

        let flushed = parser.flush();
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap().data, "incomplete");
    }

    #[test]
    fn test_openai_done_marker() {
        let event = SseEvent::new("[DONE]".to_string());
        assert!(event.is_done());

        let event = SseEvent::new("regular data".to_string());
        assert!(!event.is_done());
    }

    #[test]
    fn test_json_parsing() {
        let event = SseEvent::new(r#"{"type": "message", "text": "hello"}"#.to_string());

        let json = event.as_json().unwrap();
        assert_eq!(json["type"], "message");
        assert_eq!(json["text"], "hello");
    }

    // Real-world LLM API response tests

    #[test]
    fn test_claude_api_response() {
        let mut parser = SseParser::new();
        let claude_stream = r#"event: message_start
data: {"type":"message_start","message":{"id":"msg_123","type":"message","role":"assistant","content":[],"model":"claude-3-opus-20240229"}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":" there!"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_stop
data: {"type":"message_stop"}

"#;

        let events = parser.parse_str(claude_stream);

        assert_eq!(events.len(), 6);
        assert_eq!(events[0].event_type, "message_start");
        assert_eq!(events[1].event_type, "content_block_start");
        assert_eq!(events[2].event_type, "content_block_delta");
        assert_eq!(events[3].event_type, "content_block_delta");
        assert_eq!(events[4].event_type, "content_block_stop");
        assert_eq!(events[5].event_type, "message_stop");

        // Verify JSON parsing for delta
        let delta_json = events[2].as_json().unwrap();
        assert_eq!(delta_json["delta"]["text"], "Hello");
    }

    #[test]
    fn test_openai_api_response() {
        let mut parser = SseParser::new();
        let openai_stream = r#"data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{"content":" world"},"finish_reason":null}]}

data: {"id":"chatcmpl-123","object":"chat.completion.chunk","created":1677652288,"model":"gpt-4","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}

data: [DONE]

"#;

        let events = parser.parse_str(openai_stream);

        assert_eq!(events.len(), 5);

        // Check first delta has role
        let first_json = events[0].as_json().unwrap();
        assert_eq!(first_json["choices"][0]["delta"]["role"], "assistant");

        // Check content deltas
        let hello_json = events[1].as_json().unwrap();
        assert_eq!(hello_json["choices"][0]["delta"]["content"], "Hello");

        // Check [DONE] marker
        assert!(events[4].is_done());
    }

    #[test]
    fn test_streaming_chunks_real_scenario() {
        let mut parser = SseParser::new();

        // Simulate network chunks that split mid-event
        let chunk1 = b"event: content_block_delta\ndata: {\"type\":\"content_blo";
        let chunk2 = b"ck_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"te";
        let chunk3 = b"xt\":\"Hello\"}}\n\n";

        let events1 = parser.parse_chunk(chunk1);
        assert!(events1.is_empty());

        let events2 = parser.parse_chunk(chunk2);
        assert!(events2.is_empty());

        let events3 = parser.parse_chunk(chunk3);
        assert_eq!(events3.len(), 1);
        assert_eq!(events3[0].event_type, "content_block_delta");

        let json = events3[0].as_json().unwrap();
        assert_eq!(json["delta"]["text"], "Hello");
    }

    #[test]
    fn test_iterator_adapter() {
        let chunks: Vec<Vec<u8>> = vec![
            b"data: first\n\n".to_vec(),
            b"data: second\n\n".to_vec(),
        ];

        let events: Vec<_> = chunks.into_iter().sse_events().collect();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_id_without_null() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("id: valid-id\ndata: test\n\n");
        assert_eq!(events[0].id, Some("valid-id".to_string()));
    }

    #[test]
    fn test_id_with_null_ignored() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("id: invalid\0id\ndata: test\n\n");
        assert_eq!(events[0].id, None);
    }

    #[test]
    fn test_invalid_retry_ignored() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("retry: not-a-number\ndata: test\n\n");
        assert_eq!(events[0].retry, None);
    }

    #[test]
    fn test_field_without_colon() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("data\n\n");

        // Per spec, field without colon should be treated as field with empty value
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "");
    }

    #[test]
    fn test_unknown_field_ignored() {
        let mut parser = SseParser::new();
        let events = parser.parse_str("unknown: value\ndata: test\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "test");
    }

    #[test]
    fn test_parser_reset() {
        let mut parser = SseParser::new();

        // Parse incomplete event
        parser.parse_str("data: incomplete");

        // Reset
        parser.reset();

        // Parse new event
        let events = parser.parse_str("data: fresh\n\n");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "fresh");
    }
}
