use std::{future::Future, pin::Pin};

use http::{request::Builder, HeaderName, HeaderValue};
use hyper::{body::Bytes, Body, Response};

use super::client::Client; // ::client::Client;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("Hyper error: {0}")]
    HyperError(#[from] hyper::Error),
    #[error("Http error: {0}")]
    HttpError(#[from] http::Error),
    #[cfg(feature = "http-client-json")]
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

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

    pub async fn send(self) -> Result<Response<Body>, RequestError> {
        let req = self
            .req_builder
            .header(http::header::USER_AGENT, "fluvio-mini-http/0.1")
            .body(hyper::Body::empty())?;
        Ok(self.client.hyper.request(req).await?)
    }
}

// TODO: prefer static-dispatch once AFIT got stabilized in Rust v1.75
type ResponseExtFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

pub trait ResponseExt {
    fn bytes(self) -> ResponseExtFuture<Result<Bytes, RequestError>>;

    #[cfg(feature = "http-client-json")]
    fn json<T: serde::de::DeserializeOwned>(self) -> ResponseExtFuture<Result<T, RequestError>>
    where
        Self: Sized + Send + 'static,
    {
        let fut = async move {
            let bytes = self.bytes().await?;
            Ok(serde_json::from_slice(&bytes)?)
        };

        Box::pin(fut)
    }
}

impl<T> ResponseExt for Response<T>
where
    T: hyper::body::HttpBody + Send + 'static,
    T::Data: Send,
    RequestError: From<<T as hyper::body::HttpBody>::Error>,
{
    fn bytes(self) -> ResponseExtFuture<Result<Bytes, RequestError>> {
        let fut = async move { Ok(hyper::body::to_bytes(self.into_body()).await?) };

        Box::pin(fut)
    }
}
