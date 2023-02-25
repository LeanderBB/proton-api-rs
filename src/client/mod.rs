//! This module deals with http client and the REST requests.
mod api;
mod client_builder;
mod error;
mod http_client;
mod types;

pub use api::*;
pub use client_builder::*;
pub use error::*;
pub(crate) use http_client::*;

const DEFAULT_HOST_URL: &str = "https://mail.proton.me/api";
const DEFAULT_APP_VERSION: &str = "proton-api-rs";
const X_PM_APP_VERSION_HEADER: &str = "X-Pm-Appversion";
const X_PM_UID_HEADER: &str = "X-Pm-Uid";
