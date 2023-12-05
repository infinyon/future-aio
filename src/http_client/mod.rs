mod async_std_compat;
pub mod client;
mod request;

pub use client::Client;
pub use hyper::StatusCode;
pub use request::ResponseExt;

use hyper::{Body, Response};

use self::request::RequestBuilder;

pub async fn get(uri: impl AsRef<str>) -> Result<Response<Body>, anyhow::Error> {
    Client::new().get(uri)?.send().await
}

/// B: Response body type
pub async fn send<B>(req: &http::Request<B>) -> Result<Response<Body>, anyhow::Error> {
    let client = Client::new();
    let mut builder = http::request::Builder::new().uri(req.uri());
    let hdrs = req.headers();
    if !hdrs.is_empty() {
        for (key, value) in hdrs.iter() {
            builder = builder.header(key, value);
        }
    }
    let rb = RequestBuilder::new(client, builder);
    rb.send().await
}
