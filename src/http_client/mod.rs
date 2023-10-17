mod async_std_compat;
pub mod client;
mod request;

pub use client::Client;
pub use hyper::StatusCode;
pub use request::{RequestError, ResponseExt};

use hyper::{Body, Response};

pub async fn get(uri: impl AsRef<str>) -> Result<Response<Body>, RequestError> {
    Client::new().get(uri).send().await
}
