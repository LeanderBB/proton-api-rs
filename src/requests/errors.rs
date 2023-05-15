use serde::Deserialize;
use thiserror::Error;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct APIErrorDesc {
    pub code: u32,
    pub error: Option<String>,
}

/// Representation of the Proton API Error.
#[derive(Debug, Error)]
pub struct APIError {
    /// Http Code for the error.
    pub http_code: u16,
    /// Internal API code. Unfortunately, there is no public documentation for these values.
    pub api_code: u32,
    /// Optional error message that may be present.
    pub message: Option<String>,
}

impl std::fmt::Display for APIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(m) = &self.message {
            m.fmt(f)
        } else {
            write!(f, "APIError code={} http={}", self.api_code, self.http_code)
        }
    }
}

impl APIError {
    pub fn new(http_status: u16) -> Self {
        Self {
            http_code: http_status,
            api_code: 0,
            message: None,
        }
    }

    pub fn with_status_and_body(http_status: u16, body: &[u8]) -> Self {
        if body.is_empty() {
            return Self::new(http_status);
        }

        match serde_json::from_slice::<APIErrorDesc>(body) {
            Ok(e) => Self {
                http_code: http_status,
                api_code: e.code,
                message: e.error,
            },
            Err(_) => Self::new(http_status),
        }
    }
}
