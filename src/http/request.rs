use crate::http::{ClientAsync, ClientRequestBuilder, ClientSync, Error, FromResponse, Method};
use bytes::Bytes;
use serde::Serialize;
use std::collections::HashMap;
#[cfg(not(feature = "async-traits"))]
use std::future::Future;
use std::marker::PhantomData;
#[cfg(not(feature = "async-traits"))]
use std::pin::Pin;

/// HTTP Request representation.
#[derive(Debug)]
pub struct RequestData {
    #[allow(unused)] // Only used by http implementations.
    pub(super) method: Method,
    #[allow(unused)] // Only used by http implementations.
    pub(super) url: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Option<Bytes>,
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

    pub fn bytes(mut self, bytes: impl Into<Bytes>) -> Self {
        self.body = Some(bytes.into());
        self
    }

    pub fn json(self, value: impl Serialize) -> Self {
        let bytes = serde_json::to_vec(&value).expect("Failed to serialize json");
        self.json_bytes(bytes)
    }

    pub fn json_bytes(mut self, bytes: impl Into<Bytes>) -> Self {
        self.body = Some(bytes.into());
        self.header("Content-Type", "application/json")
    }
}

pub trait RequestDesc {
    type Output: Sized;
    type Response: FromResponse<Output = Self::Output>;

    fn build(&self) -> RequestData;

    fn to_request(&self) -> RequestWrapper<Self::Response> {
        let data = self.build();
        RequestWrapper(data, PhantomData)
    }
}

pub struct RequestWrapper<F: FromResponse>(RequestData, PhantomData<F>);

impl<F: FromResponse> Request for RequestWrapper<F> {
    type Response = F;

    fn build<C: ClientRequestBuilder>(&self, builder: &C) -> C::Request {
        builder.new_request(&self.0)
    }
}

#[cfg(not(feature = "async-traits"))]
type RequestFuture<'a, F> =
    Pin<Box<dyn Future<Output = Result<<F as FromResponse>::Output, Error>> + 'a>>;

pub trait Request {
    type Response: FromResponse;

    fn build<C: ClientRequestBuilder>(&self, builder: &C) -> C::Request;

    fn exec_sync<T: ClientSync>(
        &self,
        client: &T,
    ) -> Result<<Self::Response as FromResponse>::Output, Error> {
        client.execute::<Self::Response>(self.build(client))
    }

    #[cfg(not(feature = "async-traits"))]
    fn exec_async<'a, T: ClientAsync>(
        &'a self,
        client: &'a T,
    ) -> RequestFuture<'a, Self::Response> {
        let v = self.build(client);
        Box::pin(async move { client.execute_async::<Self::Response>(v).await })
    }

    #[cfg(feature = "async-traits")]
    async fn exec_async<'a, T: ClientAsync>(
        &'a self,
        client: &'a T,
    ) -> Result<<Self::Response as FromResponse>::Output, Error> {
        let v = self.build(client);
        client.execute_async::<Self::Response>(v).await
    }
}