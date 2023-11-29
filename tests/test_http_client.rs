#[cfg(feature = "http-client")]
#[cfg(test)]
mod test_http_client {
    use anyhow::Error;
    use fluvio_future::http_client::{self, ResponseExt, StatusCode};
    use fluvio_future::test_async;

    static SERVER: &str = "https://127.0.0.1:7879";

    #[test_async]
    async fn simple_test() -> Result<(), Error> {
        let res = http_client::get(SERVER).await;

        let status = res
            .expect("failed to get http-server, did you install and run it?")
            .status();
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[test_async]
    async fn get_and_deserialize_to_struct() -> Result<(), Error> {
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
        Ok(())
    }

    // ignored tests used for live local dev sanity check
    // cargo test live -- --ignored
    #[test_async(ignore)]
    async fn live_https() -> Result<(), Error> {
        let res = http_client::get("https://hub.infinyon.cloud").await;

        assert!(res.is_ok());
        Ok(())
    }

    #[test_async(ignore)]
    async fn live_http_not_supported() -> Result<(), Error> {
        let res = http_client::get("http://hub.infinyon.cloud").await;

        assert!(res.is_err());
        Ok(())
    }
}
