use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::flow::{HTTPFlow, FlowType};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Filter {
    pub name: String,
    pub expression: String,
    pub compiled: CompiledFilter,
}

#[derive(Debug, Clone)]
pub enum CompiledFilter {
    Always,
    Never,
    Method(String),
    Host(Regex),
    Path(Regex),
    Body(Regex),
    Header { name: String, pattern: Regex },
    StatusCode(u16),
    ContentType(Regex),
    Url(Regex),
    Error,
    Marked,
    Http,
    Tcp,
    Udp,
    WebSocket,
    And(Box<CompiledFilter>, Box<CompiledFilter>),
    Or(Box<CompiledFilter>, Box<CompiledFilter>),
    Not(Box<CompiledFilter>),
}

impl Filter {
    pub fn new(name: String, expression: String) -> Result<Self> {
        let compiled = Self::compile(&expression)?;
        Ok(Self {
            name,
            expression,
            compiled,
        })
    }

    pub fn matches(&self, flow: &HTTPFlow) -> bool {
        self.compiled.matches(flow)
    }

    fn compile(expr: &str) -> Result<CompiledFilter> {
        let expr = expr.trim();

        if expr.is_empty() {
            return Ok(CompiledFilter::Always);
        }

        // Handle logical operators
        if let Some(or_pos) = find_operator(expr, "|") {
            let left = Self::compile(&expr[..or_pos])?;
            let right = Self::compile(&expr[or_pos + 1..])?;
            return Ok(CompiledFilter::Or(Box::new(left), Box::new(right)));
        }

        if let Some(and_pos) = find_operator(expr, "&") {
            let left = Self::compile(&expr[..and_pos])?;
            let right = Self::compile(&expr[and_pos + 1..])?;
            return Ok(CompiledFilter::And(Box::new(left), Box::new(right)));
        }

        // Handle negation
        if expr.starts_with('!') {
            let inner = Self::compile(&expr[1..])?;
            return Ok(CompiledFilter::Not(Box::new(inner)));
        }

        // Handle parentheses
        if expr.starts_with('(') && expr.ends_with(')') {
            return Self::compile(&expr[1..expr.len() - 1]);
        }

        // Handle specific filters
        if expr.starts_with("~m ") {
            let method = expr[3..].trim().to_uppercase();
            return Ok(CompiledFilter::Method(method));
        }

        if expr.starts_with("~d ") {
            let pattern = expr[3..].trim();
            let regex = Regex::new(pattern).map_err(|e| Error::filter(format!("Invalid regex: {}", e)))?;
            return Ok(CompiledFilter::Host(regex));
        }

        if expr.starts_with("~u ") {
            let pattern = expr[3..].trim();
            let regex = Regex::new(pattern).map_err(|e| Error::filter(format!("Invalid regex: {}", e)))?;
            return Ok(CompiledFilter::Url(regex));
        }

        if expr.starts_with("~b ") {
            let pattern = expr[3..].trim();
            let regex = Regex::new(pattern).map_err(|e| Error::filter(format!("Invalid regex: {}", e)))?;
            return Ok(CompiledFilter::Body(regex));
        }

        if expr.starts_with("~h ") {
            let rest = expr[3..].trim();
            if let Some(colon_pos) = rest.find(':') {
                let header_name = rest[..colon_pos].trim().to_lowercase();
                let pattern = rest[colon_pos + 1..].trim();
                let regex = Regex::new(pattern).map_err(|e| Error::filter(format!("Invalid regex: {}", e)))?;
                return Ok(CompiledFilter::Header {
                    name: header_name,
                    pattern: regex,
                });
            }
        }

        if expr.starts_with("~c ") {
            let code_str = expr[3..].trim();
            if let Ok(code) = code_str.parse::<u16>() {
                return Ok(CompiledFilter::StatusCode(code));
            }
        }

        if expr.starts_with("~t ") {
            let pattern = expr[3..].trim();
            let regex = Regex::new(pattern).map_err(|e| Error::filter(format!("Invalid regex: {}", e)))?;
            return Ok(CompiledFilter::ContentType(regex));
        }

        // Handle simple keywords
        match expr {
            "~e" => Ok(CompiledFilter::Error),
            "~marked" => Ok(CompiledFilter::Marked),
            "~http" => Ok(CompiledFilter::Http),
            "~tcp" => Ok(CompiledFilter::Tcp),
            "~udp" => Ok(CompiledFilter::Udp),
            "~websocket" => Ok(CompiledFilter::WebSocket),
            _ => {
                // Try to parse as a simple regex for URL matching
                let regex = Regex::new(expr).map_err(|e| Error::filter(format!("Invalid filter expression: {}", e)))?;
                Ok(CompiledFilter::Url(regex))
            }
        }
    }
}

impl CompiledFilter {
    pub fn matches(&self, flow: &HTTPFlow) -> bool {
        match self {
            CompiledFilter::Always => true,
            CompiledFilter::Never => false,

            CompiledFilter::Method(method) => {
                flow.request.method.to_uppercase() == *method
            }

            CompiledFilter::Host(regex) => {
                regex.is_match(&flow.request.host)
            }

            CompiledFilter::Path(regex) => {
                regex.is_match(&flow.request.path)
            }

            CompiledFilter::Body(regex) => {
                if let Some(content) = &flow.request.content {
                    if let Ok(text) = String::from_utf8(content.clone()) {
                        if regex.is_match(&text) {
                            return true;
                        }
                    }
                }
                if let Some(response) = &flow.response {
                    if let Some(content) = &response.content {
                        if let Ok(text) = String::from_utf8(content.clone()) {
                            return regex.is_match(&text);
                        }
                    }
                }
                false
            }

            CompiledFilter::Header { name, pattern } => {
                // Check request headers
                for (header_name, header_value) in &flow.request.headers {
                    if header_name.to_lowercase() == *name && pattern.is_match(header_value) {
                        return true;
                    }
                }
                // Check response headers
                if let Some(response) = &flow.response {
                    for (header_name, header_value) in &response.headers {
                        if header_name.to_lowercase() == *name && pattern.is_match(header_value) {
                            return true;
                        }
                    }
                }
                false
            }

            CompiledFilter::StatusCode(code) => {
                flow.response.as_ref().map_or(false, |r| r.status_code == *code)
            }

            CompiledFilter::ContentType(regex) => {
                // Check request content-type
                for (name, value) in &flow.request.headers {
                    if name.to_lowercase() == "content-type" && regex.is_match(value) {
                        return true;
                    }
                }
                // Check response content-type
                if let Some(response) = &flow.response {
                    for (name, value) in &response.headers {
                        if name.to_lowercase() == "content-type" && regex.is_match(value) {
                            return true;
                        }
                    }
                }
                false
            }

            CompiledFilter::Url(regex) => {
                regex.is_match(&flow.request.url())
            }

            CompiledFilter::Error => flow.flow.error.is_some(),

            CompiledFilter::Marked => !flow.flow.marked.is_empty(),

            CompiledFilter::Http => matches!(flow.flow.flow_type, FlowType::Http),
            CompiledFilter::Tcp => matches!(flow.flow.flow_type, FlowType::Tcp),
            CompiledFilter::Udp => matches!(flow.flow.flow_type, FlowType::Udp),
            CompiledFilter::WebSocket => flow.websocket.is_some(),

            CompiledFilter::And(left, right) => {
                left.matches(flow) && right.matches(flow)
            }

            CompiledFilter::Or(left, right) => {
                left.matches(flow) || right.matches(flow)
            }

            CompiledFilter::Not(inner) => !inner.matches(flow),
        }
    }
}

// Helper function to find logical operators at the top level (not inside parentheses)
fn find_operator(expr: &str, op: &str) -> Option<usize> {
    let mut depth = 0;
    let mut chars = expr.char_indices().peekable();

    while let Some((i, ch)) = chars.next() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => {
                if expr[i..].starts_with(op) {
                    return Some(i);
                }
            }
            _ => {}
        }
    }

    None
}

pub fn get_filter_help() -> HashMap<&'static str, &'static str> {
    let mut help = HashMap::new();

    help.insert("~a", "Asset content-type");
    help.insert("~b", "Body");
    help.insert("~bq", "Body request");
    help.insert("~bs", "Body response");
    help.insert("~c", "Code");
    help.insert("~d", "Domain");
    help.insert("~dst", "Destination address");
    help.insert("~e", "Error");
    help.insert("~h", "Header");
    help.insert("~hq", "Header request");
    help.insert("~hs", "Header response");
    help.insert("~http", "HTTP flow");
    help.insert("~m", "Method");
    help.insert("~marked", "Marked flow");
    help.insert("~q", "Request");
    help.insert("~s", "Response");
    help.insert("~src", "Source address");
    help.insert("~t", "Content-type");
    help.insert("~tcp", "TCP flow");
    help.insert("~tq", "Content-type request");
    help.insert("~ts", "Content-type response");
    help.insert("~u", "URL");
    help.insert("~udp", "UDP flow");
    help.insert("~websocket", "WebSocket flow");

    help
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::{HTTPRequest, HTTPResponse};

    fn create_test_flow() -> HTTPFlow {
        let request = HTTPRequest::new(
            "GET".to_string(),
            "https".to_string(),
            "example.com".to_string(),
            443,
            "/api/test".to_string(),
        );
        HTTPFlow::new(request)
    }

    #[test]
    fn test_method_filter() {
        let flow = create_test_flow();
        let filter = Filter::new("test".to_string(), "~m GET".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let filter = Filter::new("test".to_string(), "~m POST".to_string()).unwrap();
        assert!(!filter.matches(&flow));
    }

    #[test]
    fn test_domain_filter() {
        let flow = create_test_flow();
        let filter = Filter::new("test".to_string(), "~d example".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let filter = Filter::new("test".to_string(), "~d google".to_string()).unwrap();
        assert!(!filter.matches(&flow));
    }

    #[test]
    fn test_url_filter() {
        let flow = create_test_flow();
        let filter = Filter::new("test".to_string(), "~u /api".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let filter = Filter::new("test".to_string(), "~u /admin".to_string()).unwrap();
        assert!(!filter.matches(&flow));
    }

    #[test]
    fn test_logical_operators() {
        let flow = create_test_flow();

        // AND operator
        let filter = Filter::new("test".to_string(), "~m GET & ~d example".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let filter = Filter::new("test".to_string(), "~m POST & ~d example".to_string()).unwrap();
        assert!(!filter.matches(&flow));

        // OR operator
        let filter = Filter::new("test".to_string(), "~m POST | ~d example".to_string()).unwrap();
        assert!(filter.matches(&flow));

        // NOT operator
        let filter = Filter::new("test".to_string(), "! ~m POST".to_string()).unwrap();
        assert!(filter.matches(&flow));
    }

    #[test]
    fn test_status_code_filter() {
        let mut flow = create_test_flow();
        flow.response = Some(HTTPResponse::new(200, "OK".to_string()));

        let filter = Filter::new("test".to_string(), "~c 200".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let filter = Filter::new("test".to_string(), "~c 404".to_string()).unwrap();
        assert!(!filter.matches(&flow));
    }

    #[test]
    fn test_error_filter() {
        let mut flow = create_test_flow();
        flow.flow.set_error("Connection failed".to_string());

        let filter = Filter::new("test".to_string(), "~e".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let flow_no_error = create_test_flow();
        assert!(!filter.matches(&flow_no_error));
    }

    #[test]
    fn test_marked_filter() {
        let mut flow = create_test_flow();
        flow.flow.marked = "important".to_string();

        let filter = Filter::new("test".to_string(), "~marked".to_string()).unwrap();
        assert!(filter.matches(&flow));

        let flow_unmarked = create_test_flow();
        assert!(!filter.matches(&flow_unmarked));
    }
}