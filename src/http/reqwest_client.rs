use crate::http::{
    ClientAsync, ClientBuilder, Error, Method, Request, Response, X_PM_APP_VERSION_HEADER,
};
use crate::requests::APIError;
use reqwest;
use std::future::Future;
use std::pin::Pin;

pub struct ReqwestClient {
    client: reqwest::Client,
    base_url: String,
}

impl TryFrom<ClientBuilder> for ReqwestClient {
    type Error = anyhow::Error;

    fn try_from(value: ClientBuilder) -> Result<Self, Self::Error> {
        use reqwest::tls::Version;
        let mut header_map = reqwest::header::HeaderMap::new();
        header_map.insert(
            X_PM_APP_VERSION_HEADER,
            reqwest::header::HeaderValue::from_str(&value.app_version)
                .map_err(|e| anyhow::anyhow!(e))?,
        );

        let mut builder = reqwest::ClientBuilder::new();

        if let Some(proxy) = value.proxy_url {
            let proxy = reqwest::Proxy::all(proxy.as_url())?;
            builder = builder.proxy(proxy);
        }

        if let Some(d) = value.connect_timeout {
            builder = builder.connect_timeout(d)
        }

        if let Some(d) = value.request_timeout {
            builder = builder.timeout(d)
        }

        builder = builder
            .min_tls_version(Version::TLS_1_2)
            .https_only(true)
            .cookie_store(true)
            .user_agent(value.user_agent)
            .default_headers(header_map);

        Ok(Self {
            client: builder.build()?,
            base_url: value.base_url,
        })
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        // Check timeout before all other errors as it can be produced by multiple
        // reqwest error kinds.
        if value.is_timeout() {
            return Error::Timeout(anyhow::Error::new(value));
        }

        if value.is_connect() {
            return Error::Connection(anyhow::Error::new(value));
        }

        if value.is_body() {
            Error::Request(anyhow::Error::new(value))
        } else if value.is_redirect() {
            Error::Redirect(
                value
                    .url()
                    .map(|v| v.to_string())
                    .unwrap_or("Unknown URL".to_string()),
                anyhow::Error::new(value),
            )
        } else if value.is_request() {
            Error::Request(anyhow::Error::new(value))
        } else {
            Error::Other(anyhow::Error::new(value))
        }
    }
}

impl ClientAsync for ReqwestClient {
    fn execute_async(
        &self,
        request: &Request,
    ) -> Pin<Box<dyn Future<Output = crate::http::Result<Response>>>> {
        let final_url = format!("{}/{}", self.base_url, request.url);

        let mut rrequest = match request.method {
            Method::Delete => self.client.delete(&final_url),
            Method::Get => self.client.get(&final_url),
            Method::Put => self.client.put(&final_url),
            Method::Post => self.client.post(&final_url),
            Method::Patch => self.client.patch(&final_url),
        };

        // Set headers.
        for (header, value) in &request.headers {
            rrequest = rrequest.header(header, value);
        }

        if let Some(body) = &request.body {
            rrequest = rrequest.body(body.to_vec())
        }

        let skips_body = request.skip_response_body;
        Box::pin(async move {
            let response = rrequest.send().await?;

            let status = response.status().as_u16();

            if status >= 400 {
                let body = response
                    .bytes()
                    .await
                    .map_err(|_| Error::API(APIError::new(status)))?;

                return Err(Error::API(APIError::with_status_and_body(
                    status,
                    body.as_ref(),
                )));
            }

            if skips_body {
                return Ok(Response { status, body: None });
            }

            let body = response.bytes().await?;

            Ok(Response {
                status,
                body: Some(body.to_vec()),
            })
        })
    }
}
