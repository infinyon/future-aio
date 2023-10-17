use fluvio_future::http_client::{self, ResponseExt, StatusCode};

#[async_std::test]
async fn simple_test() {
    let res = http_client::get("https://infinyon.com").await;

    assert!(res.is_ok());
    let status = res.unwrap().status();
    assert_eq!(status, StatusCode::OK);
}

#[async_std::test]
async fn get_and_deserialize_to_struct() {
    use serde::Deserialize;

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct Ip {
        origin: String,
    }

    let json = http_client::get("https://httpbin.org/ip")
        .await
        .expect("could not get httpbin API")
        .json::<Ip>()
        .await;

    assert!(json.is_ok());
}

#[async_std::test]
async fn get_dynamic_json() {
    use std::collections::HashMap;

    // mock API that returns `{"success": true}`
    let json_api = "https://www.mockbin.com/bin/1700ca28-0817-4998-a8af-e3b90e2aacc6";

    let json = http_client::get(json_api)
        .await
        .expect("could not get mockbin API")
        .json::<HashMap<String, bool>>()
        .await;

    assert!(json.is_ok());
    let body = json.unwrap();

    assert_eq!(body["success"], true);
}

#[async_std::test]
async fn http_not_supported() {
    let res = http_client::get("http://infinyon.com").await;

    assert!(res.is_err());
}
