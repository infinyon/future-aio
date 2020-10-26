#[cfg(feature = "native2_tls")]
#[cfg(unix)]
mod test {

    use std::io::Error as IoError;
    use std::net::SocketAddr;
    use std::time;

    use async_native_tls::TlsAcceptor;
    use async_native_tls::TlsConnector;
    use bytes::buf::ext::BufExt;
    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_util::sink::SinkExt;
    use log::debug;
    use tokio_util::codec::BytesCodec;
    use tokio_util::codec::Framed;
    use tokio_util::compat::FuturesAsyncReadCompatExt;

    use fluvio_future::tls::{
        AcceptorBuilder, AllTcpStream, CertBuilder, ConnectorBuilder, IdentityBuilder,
        PrivateKeyBuilder, X509PemBuilder,
    };
    use fluvio_future::net::TcpListener;
    use fluvio_future::net::TcpStream;
    use fluvio_future::test_async;
    use fluvio_future::timer::sleep;

    const CA_PATH: &str = "certs/certs/ca.crt";
    const SERVER_IDENTITY: &str = "certs/certs/server.pfx";
    const CLIENT_IDENTITY: &str = "certs/certs/client.pfx";
    const X509_SERVER: &str = "certs/certs/server.crt";
    const X509_SERVER_KEY: &str = "certs/certs/server.key";
    const X509_CLIENT: &str = "certs/certs/client.crt";
    const X509_CLIENT_KEY: &str = "certs/certs/client.key";

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }

    #[test_async]
    async fn test_native_tls_pk12() -> Result<(), IoError> {
        const PK12_PORT: u16 = 9900;

        let acceptor = AcceptorBuilder::identity(
            IdentityBuilder::from_path(SERVER_IDENTITY).expect("identity"),
        )
        .expect("identity:")
        .build()
        .expect("acceptor");

        let connector = ConnectorBuilder::identity(IdentityBuilder::from_path(CLIENT_IDENTITY)?)
            .expect("connector")
            .danger_accept_invalid_hostnames()
            .no_cert_verification()
            .build();

        test_tls(PK12_PORT, acceptor, connector)
            .await
            .expect("no client cert test failed");

        let acceptor = AcceptorBuilder::identity(
            IdentityBuilder::from_path(SERVER_IDENTITY).expect("identity"),
        )
        .expect("identity:")
        .build()
        .expect("acceptor");

        let connector = ConnectorBuilder::identity(IdentityBuilder::from_path(CLIENT_IDENTITY)?)
            .expect("connector")
            .no_cert_verification()
            .build();

        test_tls(PK12_PORT, acceptor, connector)
            .await
            .expect("client cert test fail");

        Ok(())
    }

    #[test_async]
    async fn test_native_tls_x509() -> Result<(), IoError> {
        const X500_PORT: u16 = 9910;

        let acceptor = AcceptorBuilder::identity(
            IdentityBuilder::from_x509(
                X509PemBuilder::from_path(X509_SERVER).expect("read"),
                PrivateKeyBuilder::from_path(X509_SERVER_KEY).expect("file"),
            )
            .expect("identity"),
        )
        .expect("identity:")
        .build()
        .expect("acceptor");

        let connector = ConnectorBuilder::identity(
            IdentityBuilder::from_x509(
                X509PemBuilder::from_path(X509_CLIENT).expect("read"),
                PrivateKeyBuilder::from_path(X509_CLIENT_KEY).expect("read"),
            )
            .expect("509"),
        )
        .expect("connector")
        .danger_accept_invalid_hostnames()
        .no_cert_verification()
        .build();

        test_tls(X500_PORT, acceptor, connector)
            .await
            .expect("no client cert test failed");

        let acceptor = AcceptorBuilder::identity(
            IdentityBuilder::from_x509(
                X509PemBuilder::from_path(X509_SERVER).expect("read"),
                PrivateKeyBuilder::from_path(X509_SERVER_KEY).expect("file"),
            )
            .expect("identity"),
        )
        .expect("identity:")
        .build()
        .expect("acceptor");

        let connector = ConnectorBuilder::identity(
            IdentityBuilder::from_x509(
                X509PemBuilder::from_path(X509_CLIENT).expect("read"),
                PrivateKeyBuilder::from_path(X509_CLIENT_KEY).expect("read"),
            )
            .expect("509"),
        )
        .expect("connector")
        .add_root_certificate(X509PemBuilder::from_path(CA_PATH).expect("cert"))
        .expect("root")
        .no_cert_verification() // for mac
        .build();

        test_tls(X500_PORT, acceptor, connector)
            .await
            .expect("no client cert test failed");

        Ok(())
    }

    async fn test_tls(
        port: u16,
        acceptor: TlsAcceptor,
        connector: TlsConnector,
    ) -> Result<(), IoError> {
        const TEST_ITERATION: u16 = 2;

        let addr = format!("127.0.0.1:{}", port)
            .parse::<SocketAddr>()
            .expect("parse");

        let server_ft = async {
            debug!("server: binding");
            let listener = TcpListener::bind(&addr).await.expect("listener failed");
            debug!("server: successfully binding. waiting for incoming");

            let mut incoming = listener.incoming();
            let incoming_stream = incoming.next().await.expect("incoming");

            debug!("server: got connection from client");
            let tcp_stream = incoming_stream.expect("no stream");

            let tls_stream = acceptor.accept(tcp_stream).await.unwrap();
            let mut framed = Framed::new(tls_stream.compat(), BytesCodec::new());

            for _ in 0..TEST_ITERATION {
                debug!("server: sending values to client");
                let data = vec![0x05, 0x0a, 0x63];
                framed.send(to_bytes(data)).await.expect("send failed");
                sleep(time::Duration::from_micros(1)).await;
                debug!("server: sending 2nd value to client");
                let data2 = vec![0x20, 0x11];
                framed.send(to_bytes(data2)).await.expect("2nd send failed");
            }

            // sleep 1 seconds so we don't lost connection
            sleep(time::Duration::from_secs(1)).await;

            Ok(()) as Result<(), IoError>
        };

        let client_ft = async {
            debug!("client: sleep to give server chance to come up");
            sleep(time::Duration::from_millis(100)).await;
            debug!("client: trying to connect");
            let tcp_stream = TcpStream::connect(&addr).await.expect("connection fail");
            let tls_stream = connector
                .connect("localhost", tcp_stream)
                .await
                .expect("tls failed");
            let all_stream = AllTcpStream::Tls(tls_stream);
            let mut framed = Framed::new(all_stream.compat(), BytesCodec::new());
            debug!("client: got connection. waiting");

            for i in 0..TEST_ITERATION {
                let value = framed.next().await.expect("frame");
                debug!("{} client :received first value from server", i);
                let bytes = value.expect("invalid value");
                let values = bytes.take(3).into_inner();
                assert_eq!(values[0], 0x05);
                assert_eq!(values[1], 0x0a);
                assert_eq!(values[2], 0x63);
                assert_eq!(values.len(), 3);

                let value2 = framed.next().await.expect("frame");
                debug!("client: received 2nd value from server");
                let bytes = value2.expect("packet decoding works");
                let values = bytes.take(2).into_inner();
                assert_eq!(values.len(), 2);
            }

            Ok(()) as Result<(), IoError>
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
