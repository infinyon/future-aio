#[cfg(all(any(unix, windows), feature = "http-client"))]
#[cfg(test)]
mod test_http_client {
    use anyhow::{Error, Result};

    use fluvio_future::http_client::{self, ResponseExt, StatusCode};
    use fluvio_future::test_async;

    static DEF_PORT: &str = "7878";
    static SERVER: &str = "https://127.0.0.1";
    static ENV_TEST_PORT: &str = "TEST_PORT";

    fn https_server_url() -> Result<String> {
        let port = std::env::var(ENV_TEST_PORT).unwrap_or(DEF_PORT.to_string());
        let port: u16 = port.parse()?;
        let port = port + 1; // http -> https
        Ok(format!("{SERVER}:{port}"))
    }

    #[test_async]
    async fn simple_test() -> Result<(), Error> {
        let server_url = https_server_url()?;
        let res = http_client::get(&server_url).await;

        let failmsg =
            format!("failed to get http-server, did you install and run it? {server_url}");
        let status = res.expect(&failmsg).status();
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[test_async]
    async fn get_and_deserialize_to_struct() -> Result<(), Error> {
        use std::net::{IpAddr, Ipv4Addr};

        use serde::Deserialize;

        let server_url = https_server_url()?;

        #[allow(dead_code)]
        #[derive(Deserialize, Debug, PartialEq)]
        struct Ip {
            origin: IpAddr,
        }

        let failmsg =
            format!("failed to get http-server, did you install and run it? {server_url}");
        let json = http_client::get(format!("{server_url}/test-data/http-client/ip.json"))
            .await
            .expect(&failmsg)
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

    #[test_async]
    async fn get_with_header() -> Result<(), Error> {
        let server_url = https_server_url()?;
        let uri = format!("{server_url}/test-data/plain.txt");
        let htreq = http_client::Client::new().get(uri)?.header("foo", "bar");
        let resp = htreq.send().await?;
        let body = resp.bytes().await?;
        let body_str = std::str::from_utf8(&body)?;
        assert_eq!("plain", body_str);
        Ok(())
    }

    #[test_async]
    async fn get_body_string() -> Result<(), Error> {
        let server_url = https_server_url()?;
        let uri = format!("{server_url}/test-data/plain.txt");
        let htreq = http_client::Client::new().get(uri)?;
        let resp = htreq.send().await?;
        let body = resp.body_string().await?;
        assert_eq!("plain", &body);
        Ok(())
    }

    #[test_async]
    async fn send_get() -> Result<(), Error> {
        let server_url = https_server_url()?;
        let failmsg =
            format!("failed to get http-server, did you install and run it? {server_url}");

        let uri = format!("{server_url}/test-data/plain.txt");
        let htreq = http::Request::get(&uri).header("foo", "bar").body("")?;

        let resp = http_client::send(htreq).await.expect(&failmsg);

        let body = resp.bytes().await?;
        let body_str = std::str::from_utf8(&body)?;
        assert_eq!("plain", body_str);
        Ok(())
    }

    #[test_async]
    async fn send_put() -> Result<(), Error> {
        let server_url = https_server_url()?;
        let failmsg =
            format!("failed to get http-server, did you install and run it? {server_url}");

        let put_txt_content = "dataput";
        let uri = format!("{server_url}/test-put/put.txt");
        let htreq = http::Request::put(&uri).body(put_txt_content)?;

        let resp = http_client::send(htreq).await.expect(&failmsg);
        // expected output from test server PUT, if it request was formtted
        // and sent this ok for now
        assert_eq!(resp.status(), http::StatusCode::METHOD_NOT_ALLOWED);
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
