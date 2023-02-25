use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Debug, Error)]
/// Error that may occur while making an HTTP request or parsing the HTTP response.
pub enum RequestError {
    #[error("An http client error occurred: {0}")]
    HttpClient(#[from] HttpClientError),
    #[error("An API error occurred: {0}")]
    API(#[from] APIError),
    #[error("A json error occurred: {0}")]
    JSON(#[from] serde_json::Error),
    #[error("Unexpected error occurred: {0}")]
    Other(#[source] anyhow::Error),
}

#[derive(Debug, Error)]
/// Errors that may occur during an HTTP request, mostly related to network.
pub enum HttpClientError {
    #[error("A redirect error occurred at '{0}")]
    Redirect(String),
    #[error("Connection timed out")]
    Timeout,
    #[error("Connection error occurred")]
    Connection,
    #[error("An error occurred related to either the request or response body")]
    Body,
    #[error("An error occurred preparing the request")]
    Request,
    #[error("Unexpected error occurred: {0}")]
    Other(#[source] anyhow::Error),
}

#[derive(Debug, Error)]
/// Representation of the Proton API Error.
pub struct APIError {
    /// Http Code for the error.
    pub http_code: u16,
    /// Internal API code. Unfortunately, there is no public documentation for these values.
    pub api_code: u32,
    /// Optional error message that may be present.
    pub message: Option<String>,
}

impl Display for APIError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(m) = &self.message {
            m.fmt(f)
        } else {
            write!(f, "APIError code={} http={}", self.api_code, self.http_code)
        }
    }
}

#[macro_export(crate)]
macro_rules! impl_error_conversion {
    ($t:ident) => {
        impl From<$crate::APIError> for $t {
            fn from(v: $crate::APIError) -> Self {
                Self::Request(v.into())
            }
        }

        impl From<$crate::HttpClientError> for $t {
            fn from(v: $crate::HttpClientError) -> Self {
                Self::Request(v.into())
            }
        }

        impl From<serde_json::Error> for $t {
            fn from(v: serde_json::Error) -> Self {
                Self::Request(v.into())
            }
        }
    };
}
