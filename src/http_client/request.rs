use std::{fmt, future::Future, pin::Pin};

use anyhow::{anyhow, Result};
use futures_util::TryFutureExt;
use http::{request::Builder, HeaderName, HeaderValue};
use hyper::{body::Bytes, Body, Response};

use super::client::Client;

pub struct RequestBuilder {
    client: Client,
    req_builder: Builder,
}

impl RequestBuilder {
    pub fn new(client: Client, req_builder: Builder) -> Self {
        Self {
            client,
            req_builder,
        }
    }

    pub fn header<K, V>(mut self, key: K, value: V) -> RequestBuilder
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.req_builder = self.req_builder.header(key, value);
        self
    }

    pub async fn send(self) -> Result<Response<Body>> {
        let req = self
            .req_builder
            .header(http::header::USER_AGENT, "fluvio-mini-http/0.1")
            .body(hyper::Body::empty())
            .map_err(|err| anyhow!("hyper error: {err:?}"))?;
        self.client
            .hyper
            .request(req)
            .map_err(|err| anyhow!("request error: {err:?}"))
            .await
    }
}

// TODO: prefer static-dispatch once AFIT got stabilized in Rust v1.75
type ResponseExtFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ResponseExt {
    fn bytes(self) -> ResponseExtFuture<Result<Bytes>>;

    #[cfg(feature = "http-client-json")]
    fn json<T: serde::de::DeserializeOwned>(self) -> ResponseExtFuture<Result<T>>
    where
        Self: Sized + Send + 'static,
    {
        let fut = async move {
            let bytes = self.bytes().await?;
            serde_json::from_slice(&bytes).map_err(|err| anyhow!("serialization error: {err:?}"))
        };

        Box::pin(fut)
    }
}

impl<T> ResponseExt for Response<T>
where
    T: hyper::body::HttpBody + Send + 'static,
    T::Data: Send,
    T::Error: fmt::Debug,
{
    fn bytes(self) -> ResponseExtFuture<Result<Bytes>> {
        let fut = async move {
            hyper::body::to_bytes(self.into_body())
                .map_err(|err| anyhow!("{err:?}"))
                .await
        };

        Box::pin(fut)
    }
}
