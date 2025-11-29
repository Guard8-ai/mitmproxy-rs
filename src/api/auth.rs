use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub authenticated: bool,
    pub token: Option<String>,
}

pub async fn auth_middleware(
    State(config): State<Arc<Config>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !config.auth_enabled {
        // Authentication disabled, allow all requests
        return Ok(next.run(request).await);
    }

    let auth_context = validate_auth(&headers, &config)?;

    // Add auth context to request extensions
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

fn validate_auth(headers: &HeaderMap, config: &Config) -> Result<AuthContext, StatusCode> {
    // Check Authorization header
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if is_valid_token(token, config) {
                    return Ok(AuthContext {
                        authenticated: true,
                        token: Some(token.to_string()),
                    });
                }
            }
        }
    }

    // Check query parameter
    // Note: In a real implementation, you'd extract query params from the request
    // This is simplified for now

    // Check cookie (if present)
    if let Some(cookie_header) = headers.get("cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            // Simple cookie parsing (in production, use a proper cookie parser)
            if cookie_str.contains("mitmproxy_auth=") {
                // For now, accept any auth cookie
                return Ok(AuthContext {
                    authenticated: true,
                    token: None,
                });
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

fn is_valid_token(token: &str, config: &Config) -> bool {
    if let Some(expected_token) = &config.auth_token {
        token == expected_token
    } else {
        // If no token is configured, any non-empty token is valid
        !token.is_empty()
    }
}

pub fn create_auth_response() -> Response<String> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header("content-type", "text/html")
        .body(create_login_form())
        .unwrap()
}

fn create_login_form() -> String {
    r#"
<!DOCTYPE html>
<html>
<head>
    <title>mitmproxy-rs authentication</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .login-form { max-width: 400px; margin: 0 auto; }
        input[type="password"] { width: 100%; padding: 10px; margin: 10px 0; }
        button { background: #007cba; color: white; padding: 10px 20px; border: none; cursor: pointer; }
        .error { color: red; margin: 10px 0; }
    </style>
</head>
<body>
    <div class="login-form">
        <h2>mitmproxy-rs</h2>
        <p>Please enter your authentication token:</p>
        <form method="post">
            <input type="password" name="token" placeholder="Authentication token" required>
            <br>
            <button type="submit">Login</button>
        </form>
    </div>
</body>
</html>
    "#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_token_validation() {
        let mut config = Config::default();
        config.auth_token = Some("test-token".to_string());

        assert!(is_valid_token("test-token", &config));
        assert!(!is_valid_token("wrong-token", &config));
        assert!(!is_valid_token("", &config));
    }

    #[test]
    fn test_auth_validation() {
        let mut config = Config::default();
        config.auth_enabled = true;
        config.auth_token = Some("test-token".to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer test-token"),
        );

        let result = validate_auth(&headers, &config);
        assert!(result.is_ok());

        let auth_context = result.unwrap();
        assert!(auth_context.authenticated);
        assert_eq!(auth_context.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_auth_validation_invalid_token() {
        let mut config = Config::default();
        config.auth_enabled = true;
        config.auth_token = Some("test-token".to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer wrong-token"),
        );

        let result = validate_auth(&headers, &config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }
}