mod async_std_compat;
pub mod client;
mod request;

use anyhow::Result;
pub use client::Client;
pub use hyper::StatusCode;
pub use request::ResponseExt;

use hyper::{Body, Response};

pub const USER_AGENT: &str = "fluvio-futures-http/0.1";

/// for simple get requests
pub async fn get(uri: impl AsRef<str>) -> Result<Response<Body>> {
    Client::new().get(uri)?.send().await
}

/// for more complex http requests
/// send via a request struct
/// let req = http::Request::get(&uri)
///    .header("foo", "bar")
///    .body("")?;
/// let resp = http_client::send(&htreq).await.expect(&failmsg);
pub async fn send<B: Into<hyper::Body>>(req: http::Request<B>) -> Result<Response<Body>> {
    let client = Client::new();
    client.send(req).await
}
