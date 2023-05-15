//! UReq HTTP client implementation.

use crate::http::X_PM_APP_VERSION_HEADER;
use crate::http::{ClientBuilder, ClientSync, Error, Method, Request, Response};
use crate::requests::APIError;
use std::io;
use std::io::Read;
use ureq;

pub struct UReqClient {
    agent: ureq::Agent,
    app_version: String,
    base_url: String,
}

impl TryFrom<ClientBuilder> for UReqClient {
    type Error = anyhow::Error;

    fn try_from(value: ClientBuilder) -> Result<Self, Self::Error> {
        let mut builder = ureq::AgentBuilder::new();

        if let Some(d) = value.request_timeout {
            builder = builder.timeout(d);
        }

        if let Some(d) = value.connect_timeout {
            builder = builder.timeout_connect(d)
        }

        if let Some(proxy) = value.proxy_url {
            let proxy = ureq::Proxy::new(proxy.as_url())?;
            builder = builder.proxy(proxy);
        }

        let agent = builder
            .https_only(true)
            .user_agent(&value.user_agent)
            .max_idle_connections(0)
            .max_idle_connections_per_host(0)
            .build();

        Ok(Self {
            agent,
            app_version: value.app_version,
            base_url: value.base_url,
        })
    }
}

impl From<ureq::Error> for Error {
    fn from(value: ureq::Error) -> Self {
        match value {
            ureq::Error::Status(status, response) => {
                if let Ok(body) = safe_read_body(response) {
                    return Error::API(APIError::with_status_and_body(status, &body));
                }

                Error::API(APIError::new(status))
            }
            ureq::Error::Transport(t) => match t.kind() {
                ureq::ErrorKind::InvalidUrl => Error::Request(t.into()),
                ureq::ErrorKind::UnknownScheme => Error::Request(t.into()),
                ureq::ErrorKind::Dns => Error::Connection(t.into()),
                ureq::ErrorKind::InsecureRequestHttpsOnly => Error::Request(t.into()),
                ureq::ErrorKind::ConnectionFailed => Error::Connection(t.into()),
                ureq::ErrorKind::TooManyRedirects => Error::Redirect(
                    t.url()
                        .map(|u| u.to_string())
                        .unwrap_or("Unknown url".to_string()),
                    t.into(),
                ),
                ureq::ErrorKind::BadStatus => Error::Request(t.into()),
                ureq::ErrorKind::BadHeader => Error::Request(t.into()),
                ureq::ErrorKind::Io => Error::Connection(t.into()),
                ureq::ErrorKind::InvalidProxyUrl => Error::Connection(t.into()),
                ureq::ErrorKind::ProxyConnect => Error::Connection(t.into()),
                ureq::ErrorKind::ProxyUnauthorized => Error::Connection(t.into()),
                ureq::ErrorKind::HTTP => Error::Request(t.into()),
            },
        }
    }
}

impl ClientSync for UReqClient {
    fn execute(&self, request: &Request) -> Result<Response, Error> {
        let final_url = format!("{}/{}", self.base_url, request.url);
        let mut ureq_request = match request.method {
            Method::Delete => self.agent.delete(&final_url),
            Method::Get => self.agent.get(&final_url),
            Method::Put => self.agent.put(&final_url),
            Method::Post => self.agent.post(&final_url),
            Method::Patch => self.agent.patch(&final_url),
        };

        // Set app version.
        ureq_request = ureq_request.set(X_PM_APP_VERSION_HEADER, &self.app_version);

        // Set headers.
        for (header, value) in &request.headers {
            ureq_request = ureq_request.set(header, value);
        }

        let ureq_response = if let Some(body) = &request.body {
            ureq_request.send_bytes(body)?
        } else {
            ureq_request.call()?
        };

        let status = ureq_response.status();

        if request.skip_response_body {
            return Ok(Response { status, body: None });
        }

        let body = safe_read_body(ureq_response)
            .map_err(|e| Error::Request(anyhow::anyhow!("Failed to read response body {e}")))?;

        Ok(Response {
            status,
            body: if !body.is_empty() { Some(body) } else { None },
        })
    }
}

fn safe_read_body(response: ureq::Response) -> Result<Vec<u8>, io::Error> {
    let mut vec = vec![];

    if let Some(length) = response.header("Content-Length") {
        if let Ok(len) = length.parse::<usize>() {
            if len == 0 {
                return Ok(vec![]);
            }
            vec.reserve(len);
        }
    }

    let _ = response
        .into_reader()
        .take(10_000_000)
        .read_to_end(&mut vec)?;

    Ok(vec)
}
