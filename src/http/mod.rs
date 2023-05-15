//! Basic HTTP Protocol abstraction for the Proton API.

use crate::domain::SecretString;
use anyhow;
use secrecy::ExposeSecret;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;

#[cfg(feature = "http-ureq")]
pub mod ureq_client;

#[cfg(feature = "http-reqwest")]
pub mod reqwest_client;

pub(crate) const DEFAULT_HOST_URL: &str = "https://mail.proton.me/api";
pub(crate) const DEFAULT_APP_VERSION: &str = "proton-api-rs";
#[allow(unused)] // it is used by the http implementations
pub(crate) const X_PM_APP_VERSION_HEADER: &str = "X-Pm-Appversion";
pub(crate) const X_PM_UID_HEADER: &str = "X-Pm-Uid";

/// HTTP method.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Method {
    Delete,
    Get,
    Put,
    Post,
    Patch,
}

/// HTTP Request representation.
#[derive(Debug)]
pub struct Request {
    #[allow(unused)] // Only used by http implementations.
    pub(super) method: Method,
    #[allow(unused)] // Only used by http implementations.
    pub(super) url: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Option<Vec<u8>>,
    pub(super) skip_response_body: bool,
}

impl Request {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HashMap::new(),
            body: None,
            skip_response_body: false,
        }
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn bearer_token(self, token: &str) -> Self {
        self.header("authorization", format!("Bearer {token}"))
    }

    pub fn bytes(mut self, bytes: Vec<u8>) -> Self {
        self.body = Some(bytes);
        self
    }

    pub fn json(self, value: impl Serialize) -> Self {
        let bytes = serde_json::to_vec(&value).expect("Failed to serialize json");
        self.json_bytes(bytes)
    }

    pub fn json_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.body = Some(bytes);
        self.header("Content-Type", "application/json")
    }

    fn skip_response_body(mut self) -> Self {
        self.skip_response_body = true;
        self
    }
}

/// HTTP Response Object
pub struct Response {
    pub(super) status: u16,
    pub(super) body: Option<Vec<u8>>,
}

impl Response {
    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    pub fn as_json<T: DeserializeOwned>(&self) -> std::result::Result<T, serde_json::Error> {
        let data = if let Some(b) = &self.body {
            b.as_slice()
        } else {
            &[]
        };

        serde_json::from_slice::<T>(data)
    }
}

/// Errors that may occur during an HTTP request, mostly related to network.
#[derive(Debug, Error)]
pub enum Error {
    #[error("API Error: {0}")]
    API(#[from] crate::requests::APIError),
    #[error("A redirect error occurred at '{0}: {1}")]
    Redirect(String, #[source] anyhow::Error),
    #[error("Connection timed out")]
    Timeout(#[source] anyhow::Error),
    #[error("Connection error: {0}")]
    Connection(#[source] anyhow::Error),
    #[error("Request/Response body error: {0}")]
    Request(#[source] anyhow::Error),
    #[error("Unexpected error occurred: {0}")]
    Other(#[source] anyhow::Error),
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Request(value.into())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProxyProtocol {
    Https,
    Socks5,
}

#[derive(Debug, Clone)]
pub struct ProxyAuth {
    pub username: String,
    pub password: SecretString,
}

#[derive(Debug, Clone)]
pub struct Proxy {
    pub protocol: ProxyProtocol,
    pub auth: Option<ProxyAuth>,
    pub url: String,
    pub port: u16,
}

impl Proxy {
    pub fn as_url(&self) -> String {
        let protocol = match self.protocol {
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        };

        let auth = if let Some(auth) = &self.auth {
            format!("{}:{}", auth.username, auth.password.expose_secret())
        } else {
            String::new()
        };

        format!("{protocol}://{auth}@{}:{}", self.url, self.port)
    }
}

/// Builder for an http client
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    app_version: String,
    base_url: String,
    request_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    user_agent: String,
    proxy_url: Option<Proxy>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self {
            app_version: DEFAULT_APP_VERSION.to_string(),
            user_agent: "NoClient/0.1.0".to_string(),
            base_url: DEFAULT_HOST_URL.to_string(),
            request_timeout: None,
            connect_timeout: None,
            proxy_url: None,
        }
    }

    /// Set the app version for this client e.g.: my-client@1.4.0+beta.
    /// Note: The default app version is not guaranteed to be accepted by the proton servers.
    pub fn app_version(mut self, version: &str) -> Self {
        self.app_version = version.to_string();
        self
    }

    /// Set the user agent to be submitted with every request.
    pub fn user_agent(mut self, agent: &str) -> Self {
        self.user_agent = agent.to_string();
        self
    }

    /// Set server's base url. By default the proton API server url is used.
    pub fn base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    /// Set the full request timeout. By default there is no timeout.
    pub fn request_timeout(mut self, duration: Duration) -> Self {
        self.request_timeout = Some(duration);
        self
    }

    /// Set the connection timeout. By default there is no timeout.
    pub fn connect_timeout(mut self, duration: Duration) -> Self {
        self.connect_timeout = Some(duration);
        self
    }

    /// Specify proxy URL for the builder.
    pub fn with_proxy(mut self, proxy: Proxy) -> Self {
        self.proxy_url = Some(proxy);
        self
    }

    pub fn build<T: TryFrom<ClientBuilder, Error = anyhow::Error>>(
        self,
    ) -> std::result::Result<T, anyhow::Error> {
        T::try_from(self)
    }
}

/// Abstraction for request creation, this can enable wrapping of request creations to add
/// session token or other headers.
pub trait RequestFactory {
    fn new_request(&self, method: Method, url: &str) -> Request;
}

/// Default request factory, creates basic requests.
#[derive(Copy, Clone)]
pub struct DefaultRequestFactory {}

impl RequestFactory for DefaultRequestFactory {
    fn new_request(&self, method: Method, url: &str) -> Request {
        Request::new(method, url)
    }
}

/// RequestWithBody trait should be applied to every request object which needs to look at the body
/// of the request.
pub trait RequestWithBody {
    type Response: DeserializeOwned;

    fn build_request(&self, factory: &dyn RequestFactory) -> Request;

    fn execute_sync<C: ClientSync>(
        &self,
        client: &C,
        factory: &dyn RequestFactory,
    ) -> Result<Self::Response> {
        let request = self.build_request(factory);
        let response = client.execute(&request)?;
        let result = response.as_json::<Self::Response>()?;
        Ok(result)
    }

    fn execute_async<'a, C: ClientAsync>(
        &self,
        client: &'a C,
        factory: &dyn RequestFactory,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Response>> + 'a>> {
        let request = self.build_request(factory);
        Box::pin(async move {
            let response = client.execute_async(&request).await?;
            let result = response.as_json::<Self::Response>()?;
            Ok(result)
        })
    }
}

/// RequestNoBody trait should be implemented for all request which do not which do not need
/// to inspect the body of the request.
pub trait RequestNoBody {
    fn build_request(&self, factory: &dyn RequestFactory) -> Request;

    fn execute_sync<C: ClientSync>(&self, client: &C, factory: &dyn RequestFactory) -> Result<()> {
        let request = self.build_request(factory).skip_response_body();
        client.execute(&request)?;
        Ok(())
    }

    fn execute_async<'a, C: ClientAsync>(
        &self,
        client: &'a C,
        factory: &dyn RequestFactory,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + 'a>> {
        let request = self.build_request(factory).skip_response_body();
        Box::pin(async move {
            client.execute_async(&request).await?;
            Ok(())
        })
    }
}

/// HTTP Client abstraction Sync.
pub trait ClientSync: TryFrom<ClientBuilder, Error = anyhow::Error> {
    fn execute(&self, request: &Request) -> Result<Response>;
}

/// HTTP Client abstraction Async.
pub trait ClientAsync: TryFrom<ClientBuilder, Error = anyhow::Error> {
    fn execute_async(&self, request: &Request) -> Pin<Box<dyn Future<Output = Result<Response>>>>;
}
