mod async_std_compat;
pub mod client;
mod request;

pub use client::Client;
pub use hyper::StatusCode;
pub use request::ResponseExt;

use hyper::{Body, Response};

pub async fn get(uri: impl AsRef<str>) -> Result<Response<Body>, anyhow::Error> {
    Client::new().get(uri)?.send().await
}

#[cfg(test)]
mod tests {
    use crate::http_client::{self, ResponseExt, StatusCode};

    static SERVER: &str = "https://127.0.0.1:7879";

    #[async_std::test]
    async fn simple_test() {
        let res = http_client::get(SERVER).await;

        let status = res
            .expect("failed to get http-server, did you install and run it?")
            .status();
        assert_eq!(status, StatusCode::OK);
    }

    #[async_std::test]
    async fn get_and_deserialize_to_struct() {
        use std::net::{IpAddr, Ipv4Addr};

        use serde::Deserialize;

        #[allow(dead_code)]
        #[derive(Deserialize, Debug, PartialEq)]
        struct Ip {
            origin: IpAddr,
        }

        let json = http_client::get(format!("{SERVER}/test-data/http-client/ip.json"))
            .await
            .expect("failed to get http-server, did you install and run it?")
            .json::<Ip>()
            .await
            .expect("failed to parse IP address");

        assert_eq!(
            json,
            Ip {
                origin: IpAddr::V4(Ipv4Addr::new(192, 0, 0, 1))
            }
        );
    }

    #[async_std::test]
    async fn http_not_supported() {
        let res = http_client::get("http://infinyon.com").await;

        assert!(res.is_err());
    }
}
