use std::net::SocketAddr;
use std::time;

use anyhow::Result;
use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use futures_lite::future::zip;
use futures_lite::stream::StreamExt;
use futures_util::SinkExt;
use tokio_util::codec::BytesCodec;
use tokio_util::codec::Framed;
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::debug;

use crate::net::{TcpListener, tcp_stream::stream};
use crate::test_async;
use crate::timer::sleep;

use super::{TlsAcceptor, TlsConnector};

const CA_PATH: &str = "certs/test-certs/ca.crt";
const INTERMEDIATE_CA_PATH: &str = "certs/test-certs/intermediate-ca.crt";
const ITER: u16 = 10;

fn to_bytes(bytes: Vec<u8>) -> Bytes {
    let mut buf = BytesMut::with_capacity(bytes.len());
    buf.put_slice(&bytes);
    buf.freeze()
}

#[test_async]
async fn test_tls() -> Result<()> {
    // Test the client against a server with CA intermediary cert chain
    // Requires X509VerifyFlags::PARTIAL_CHAIN or allow_partial: true (default)
    run_test(
        TlsAcceptor::builder()?
            .with_certifiate_and_key_from_pem_files(
                "certs/test-certs/intermediate-server.crt",
                "certs/test-certs/intermediate-server.key",
            )?
            .build(),
        TlsConnector::builder()?
            .with_hostname_verification_disabled()?
            .with_ca_from_pem_file(INTERMEDIATE_CA_PATH)?
            .build(),
    )
    .await
    .expect("no client cert test with root CA with intermediaries failed");

    // Test the client against a server with CA only without intermediary certs
    run_test(
        TlsAcceptor::builder()?
            .with_certifiate_and_key_from_pem_files(
                "certs/test-certs/server.crt",
                "certs/test-certs/server.key",
            )?
            .build(),
        TlsConnector::builder()?
            .with_hostname_verification_disabled()?
            .with_ca_from_pem_file(CA_PATH)?
            .build(),
    )
    .await
    .expect("no client cert test with root CA only failed");

    // test client authentication
    run_test(
        TlsAcceptor::builder()?
            .with_certifiate_and_key_from_pem_files(
                "certs/test-certs/server.crt",
                "certs/test-certs/server.key",
            )?
            .with_ca_from_pem_file(CA_PATH)?
            .build(),
        TlsConnector::builder()?
            .with_certifiate_and_key_from_pem_files(
                "certs/test-certs/client.crt",
                "certs/test-certs/client.key",
            )?
            .with_ca_from_pem_file(CA_PATH)?
            .build(),
    )
    .await
    .expect("client cert test fail");

    Ok(())
}

async fn run_test(acceptor: TlsAcceptor, connector: TlsConnector) -> Result<()> {
    let addr = "127.0.0.1:19988".parse::<SocketAddr>().expect("parse");

    let server_ft = async {
        debug!("server: binding");
        let listener = TcpListener::bind(&addr).await.expect("listener failed");
        debug!("server: successfully binding. waiting for incoming");

        let mut incoming = listener.incoming();
        let stream = incoming.next().await.expect("stream");
        let tcp_stream = stream.expect("no stream");
        let acceptor = acceptor.clone();
        debug!("server: got connection from client");
        debug!("server: try to accept tls connection");
        let handshake = acceptor.accept(tcp_stream);

        debug!("server: handshaking");
        let tls_stream = handshake.await.expect("hand shake failed");

        let mut framed = Framed::new(tls_stream.compat(), BytesCodec::new());

        for i in 0..ITER {
            let receives_bytes = framed.next().await.expect("frame");

            let bytes = receives_bytes.expect("invalid value");
            debug!(
                "server: loop {}, received from client: {} bytes",
                i,
                bytes.len()
            );

            let slice = bytes.as_ref();
            let mut str_bytes = vec![];
            for b in slice {
                str_bytes.push(b.to_owned());
            }
            let message = String::from_utf8(str_bytes).expect("utf8");
            assert_eq!(message, format!("message{}", i));
            let resply = format!("{}reply", message);
            let reply_bytes = resply.as_bytes();
            debug!("sever: send back reply: {}", resply);
            framed
                .send(to_bytes(reply_bytes.to_vec()))
                .await
                .expect("send failed");
        }

        Ok(()) as Result<()>
    };

    let client_ft = async {
        debug!("client: sleep to give server chance to come up");
        sleep(time::Duration::from_millis(100)).await;
        debug!("client: trying to connect");
        let tcp_stream = stream(&addr).await.expect("connection fail");
        let tls_stream = connector
            .connect("localhost", tcp_stream)
            .await
            .expect("tls failed");
        let mut framed = Framed::new(tls_stream.compat(), BytesCodec::new());
        debug!("client: got connection. waiting");

        for i in 0..ITER {
            let message = format!("message{}", i);
            let bytes = message.as_bytes();
            debug!("client: loop {} sending test message", i);
            framed
                .send(to_bytes(bytes.to_vec()))
                .await
                .expect("send failed");
            let reply = framed.next().await.expect("messages").expect("frame");
            debug!("client: loop {}, received reply back", i);
            let slice = reply.as_ref();
            let mut str_bytes = vec![];
            for b in slice {
                str_bytes.push(b.to_owned());
            }
            let message = String::from_utf8(str_bytes).expect("utf8");
            assert_eq!(message, format!("message{}reply", i));
        }

        Ok(()) as Result<()>
    };

    let _ = zip(client_ft, server_ft).await;

    Ok(())
}
