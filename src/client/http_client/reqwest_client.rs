use crate::client::{DEFAULT_APP_VERSION, DEFAULT_HOST_URL, X_PM_APP_VERSION_HEADER};
use crate::{APIError, HttpClientError, RequestError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug)]
/// Reqwest http client implementation. Use `HttpClientBuilder` to create a new instance.
pub struct HttpClient {
    client: reqwest::Client,
    base_url: String,
}

/// Builder for an http client
#[derive(Clone)]
pub struct HttpClientBuilder {
    app_version: String,
    base_url: String,
    request_timeout: Duration,
    user_agent: String,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        Self {
            app_version: DEFAULT_APP_VERSION.to_string(),
            user_agent: "NoClient/0.1.0".to_string(),
            base_url: DEFAULT_HOST_URL.to_string(),
            request_timeout: Duration::from_secs(5),
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

    /// Set the request timeout. By default the timeout is set to 5 seconds.
    pub fn request_timeout(mut self, duration: Duration) -> Self {
        self.request_timeout = duration;
        self
    }

    /// Constructs the http client
    pub fn build(self) -> Result<HttpClient, HttpClientError> {
        HttpClient::new(self)
    }
}

impl HttpClient {
    fn new(http_builder: HttpClientBuilder) -> Result<Self, HttpClientError> {
        let mut header_map = reqwest::header::HeaderMap::new();
        header_map.insert(
            X_PM_APP_VERSION_HEADER,
            reqwest::header::HeaderValue::from_str(&http_builder.app_version)
                .map_err(|e| HttpClientError::Other(anyhow::format_err!(e)))?,
        );

        let builder = reqwest::ClientBuilder::new();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = {
            use reqwest::tls::Version;
            builder
                .min_tls_version(Version::TLS_1_2)
                .https_only(true)
                .cookie_store(true)
                .timeout(http_builder.request_timeout)
                .user_agent(http_builder.user_agent)
                .default_headers(header_map)
        };

        Ok(Self {
            client: builder.build()?,
            base_url: http_builder.base_url,
        })
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder(self.client.post(self.combine_url(url)))
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder(self.client.get(self.combine_url(url)))
    }

    pub fn delete(&self, url: &str) -> RequestBuilder {
        RequestBuilder(self.client.delete(self.combine_url(url)))
    }

    #[inline(always)]
    fn combine_url(&self, url: &str) -> String {
        format!("{}{}", self.base_url, url)
    }
}

/// Wrapper around Reqwest's request builder so it's not exposed directly.
pub struct RequestBuilder(reqwest::RequestBuilder);

impl RequestBuilder {
    /// Set the request JSON body.
    pub fn with_body<T: Serialize>(self, v: &T) -> Self {
        RequestBuilder(self.0.json(v))
    }

    /// Set a a request header.
    pub fn header(self, key: &str, value: &str) -> Self {
        RequestBuilder(self.0.header(key, value))
    }

    /// Set the bearer token for the request.
    pub fn bearer_token(self, value: &str) -> Self {
        RequestBuilder(self.0.bearer_auth(value))
    }

    /// Execute a request. HTTP status errors are also converted into a `RequestError`.
    pub async fn execute(self) -> Result<Response, RequestError> {
        let r = self.0.send().await?;
        if !r.status().is_success() {
            return Err(status_into_api_error(r).await);
        }

        Ok(Response(r))
    }
}

impl From<reqwest::Error> for RequestError {
    fn from(value: reqwest::Error) -> Self {
        RequestError::HttpClient(value.into())
    }
}

impl From<reqwest::Error> for HttpClientError {
    fn from(value: reqwest::Error) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if value.is_connect() {
            return HttpClientError::Connection;
        }

        if value.is_body() {
            HttpClientError::Body
        } else if value.is_redirect() {
            HttpClientError::Redirect(
                value
                    .url()
                    .map(|v| v.to_string())
                    .unwrap_or("Unknown URL".to_string()),
            )
        } else if value.is_timeout() {
            HttpClientError::Timeout
        } else if value.is_request() {
            HttpClientError::Request
        } else {
            HttpClientError::Other(anyhow::Error::new(value))
        }
    }
}

#[derive(Debug)]
/// Wrapper around reqwest::Response to avoid direct exposure.
pub struct Response(reqwest::Response);

impl Response {
    /// Return the response's status code
    #[allow(unused)]
    pub fn status_code(&self) -> u16 {
        self.0.status().as_u16()
    }

    /// Get the response's body as bytes
    pub async fn into_bytes(self) -> Result<ResponseBody, RequestError> {
        let bytes = self.0.bytes().await?;
        Ok(ResponseBody(bytes))
    }

    /// Get the response's body as JSOn and deserialize into the given type.
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, RequestError> {
        // Even though there is Response::json<>, this way we can inspect the
        // request bytes before conversion if necessary.
        let bytes = self.0.bytes().await?;
        let t = serde_json::from_slice::<T>(bytes.as_ref())?;
        Ok(t)
    }
}

#[derive(Debug)]
/// Response body bytes. Obtained through `Response::into_bytes`.
pub struct ResponseBody(bytes::Bytes);

impl AsRef<[u8]> for ResponseBody {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl ResponseBody {
    pub fn as_json<'a, T: Deserialize<'a>>(&'a self) -> Result<T, RequestError> {
        let t = serde_json::from_slice::<T>(self.as_ref())?;
        Ok(t)
    }
}

async fn status_into_api_error(response: reqwest::Response) -> RequestError {
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct APIDesc {
        code: u32,
        error: Option<String>,
    }

    let http_code = response.status().as_u16();

    let api_desc = match response.json::<APIDesc>().await {
        Ok(v) => v,
        Err(e) => return e.into(),
    };

    RequestError::API(APIError {
        http_code,
        api_code: api_desc.code,
        message: api_desc.error,
    })
}
