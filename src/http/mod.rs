//! Basic HTTP Protocol abstraction for the Proton API.

use crate::domain::SecretString;
use anyhow;
use secrecy::ExposeSecret;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
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
pub(crate) const X_PM_HUMAN_VERIFICATION_TOKEN: &str = "X-Pm-Human-Verification-Token";
pub(crate) const X_PM_HUMAN_VERIFICATION_TOKEN_TYPE: &str = "X-Pm-Human-Verification-Token-Type";

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
pub struct RequestData {
    #[allow(unused)] // Only used by http implementations.
    pub(super) method: Method,
    #[allow(unused)] // Only used by http implementations.
    pub(super) url: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Option<Vec<u8>>,
}

impl RequestData {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: HashMap::new(),
            body: None,
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
    #[error("Encoding/Decoding error: {0}")]
    EncodeOrDecode(#[source] anyhow::Error),
    #[error("Unexpected error occurred: {0}")]
    Other(#[source] anyhow::Error),
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::EncodeOrDecode(value.into())
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
            format!("{}:{}@", auth.username, auth.password.expose_secret())
        } else {
            String::new()
        };

        format!("{protocol}://{auth}{}:{}", self.url, self.port)
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
    debug: bool,
    allow_http: bool,
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
            debug: false,
            allow_http: false,
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

    /// Allow http request
    pub fn allow_http(mut self) -> Self {
        self.allow_http = true;
        self
    }

    /// Enable request debugging.
    pub fn debug(mut self) -> Self {
        self.debug = true;
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
    fn new_request(&self, method: Method, url: &str) -> RequestData;
}

/// Default request factory, creates basic requests.
#[derive(Copy, Clone)]
pub struct DefaultRequestFactory {}

impl RequestFactory for DefaultRequestFactory {
    fn new_request(&self, method: Method, url: &str) -> RequestData {
        RequestData::new(method, url)
    }
}

pub trait ResponseBodySync {
    type Body: AsRef<[u8]>;
    fn get_body(self) -> Result<Self::Body>;
}

pub trait ResponseBodyAsync {
    type Body: AsRef<[u8]>;
    fn get_body_async(self) -> Pin<Box<dyn Future<Output = Result<Self::Body>>>>;
}

pub trait FromResponse {
    type Output;
    fn from_response_sync<T: ResponseBodySync>(response: T) -> Result<Self::Output>;

    fn from_response_async<T: ResponseBodyAsync + 'static>(
        response: T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output>>>>;
}

#[derive(Copy, Clone)]
pub struct NoResponse {}

impl FromResponse for NoResponse {
    type Output = ();

    fn from_response_sync<T: ResponseBodySync>(_: T) -> Result<Self::Output> {
        Ok(())
    }

    fn from_response_async<T: ResponseBodyAsync>(
        _: T,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output>>>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct JsonResponse<T: DeserializeOwned>(PhantomData<T>);

impl<T: DeserializeOwned> FromResponse for JsonResponse<T> {
    type Output = T;

    fn from_response_sync<R: ResponseBodySync>(response: R) -> Result<Self::Output> {
        let body = response.get_body()?;
        let r = serde_json::from_slice(body.as_ref())?;
        Ok(r)
    }

    fn from_response_async<R: ResponseBodyAsync + 'static>(
        response: R,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output>>>> {
        Box::pin(async move {
            let body = response.get_body_async().await?;
            let r = serde_json::from_slice(body.as_ref())?;
            Ok(r)
        })
    }
}

#[derive(Copy, Clone)]
pub struct StringResponse {}

impl FromResponse for StringResponse {
    type Output = String;

    fn from_response_sync<R: ResponseBodySync>(response: R) -> Result<Self::Output> {
        let body = response.get_body()?;
        Ok(String::from_utf8_lossy(body.as_ref()).to_string())
    }

    fn from_response_async<R: ResponseBodyAsync + 'static>(
        response: R,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output>>>> {
        Box::pin(async move {
            let body = response.get_body_async().await?;
            Ok(String::from_utf8_lossy(body.as_ref()).to_string())
        })
    }
}

pub trait Request {
    type Output: Sized;
    type Response: FromResponse<Output = Self::Output>;

    fn build_request(&self, factory: &dyn RequestFactory) -> RequestData;

    fn execute_sync<T: ClientSync>(
        &self,
        client: &T,
        factory: &dyn RequestFactory,
    ) -> Result<Self::Output> {
        client.execute(self, factory)
    }

    fn execute_async<T: ClientAsync>(
        &self,
        client: &T,
        factory: &dyn RequestFactory,
    ) -> Pin<Box<dyn Future<Output = Result<Self::Output>>>> {
        client.execute_async(self, factory)
    }
}

/// HTTP Client abstraction Sync.
pub trait ClientSync: TryFrom<ClientBuilder, Error = anyhow::Error> {
    fn execute<R: Request + ?Sized>(
        &self,
        request: &R,
        factory: &dyn RequestFactory,
    ) -> Result<R::Output>;
}

/// HTTP Client abstraction Async.
pub trait ClientAsync: TryFrom<ClientBuilder, Error = anyhow::Error> {
    fn execute_async<R: Request + ?Sized>(
        &self,
        request: &R,
        factory: &dyn RequestFactory,
    ) -> Pin<Box<dyn Future<Output = Result<R::Output>>>>;
}
