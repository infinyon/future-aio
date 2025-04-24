use crate::net::TcpStream;

pub use async_native_tls::TlsAcceptor;
pub use async_native_tls::TlsConnector;
pub use async_native_tls::TlsStream;

// server both cliennt and server and same but use same pattern as rustls
pub type DefaultServerTlsStream = TlsStream<TcpStream>;
pub type DefaultClientTlsStream = TlsStream<TcpStream>;

pub use connector::*;

pub use crate::net::certs::CertBuilder;

mod split {

    use async_net::TcpStream;
    use futures_util::AsyncReadExt;

    use super::*;
    use crate::net::{BoxReadConnection, BoxWriteConnection, SplitConnection};

    impl SplitConnection for TlsStream<TcpStream> {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            let (read, write) = self.split();
            (Box::new(write), Box::new(read))
        }
    }
}

mod connector {
    use std::io::Error as IoError;
    use std::io::ErrorKind;
    use std::sync::Arc;

    use async_trait::async_trait;
    use tracing::debug;

    use crate::net::{
        AsConnectionFd, BoxReadConnection, BoxWriteConnection, ConnectionFd, DomainConnector,
        SplitConnection, TcpDomainConnector,
        tcp_stream::{SocketOpts, stream, stream_with_opts},
    };

    use super::*;

    /// connect as anonymous client
    #[derive(Clone)]
    pub struct TlsAnonymousConnector(Arc<TlsConnector>);

    impl From<TlsConnector> for TlsAnonymousConnector {
        fn from(connector: TlsConnector) -> Self {
            Self(Arc::new(connector))
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsAnonymousConnector {
        async fn connect(
            &self,
            domain: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError> {
            let tcp_stream = stream(domain).await?;
            let fd = tcp_stream.as_connection_fd();
            let (write, read) = self
                .0
                .connect(domain, tcp_stream)
                .await
                .map_err(|e| {
                    IoError::new(
                        ErrorKind::ConnectionRefused,
                        format!("failed to connect: {}", e),
                    )
                })?
                .split_connection();
            Ok((write, read, fd))
        }

        fn new_domain(&self, _domain: String) -> DomainConnector {
            Box::new(self.clone())
        }

        fn domain(&self) -> &str {
            "localhost"
        }

        fn clone_box(&self) -> DomainConnector {
            Box::new(self.clone())
        }
    }

    /// Connect to TLS
    #[derive(Clone)]
    pub struct TlsDomainConnector {
        domain: String,
        connector: Arc<TlsConnector>,
    }

    impl TlsDomainConnector {
        pub fn new(connector: TlsConnector, domain: String) -> Self {
            Self {
                domain,
                connector: Arc::new(connector),
            }
        }

        pub fn domain(&self) -> &str {
            &self.domain
        }

        pub fn connector(&self) -> &TlsConnector {
            &self.connector
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsDomainConnector {
        async fn connect(
            &self,
            addr: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError> {
            debug!("connect to tls addr: {}", addr);
            let socket_opts = SocketOpts {
                keepalive: Some(Default::default()),
                nodelay: Some(true),
            };
            let tcp_stream = stream_with_opts(addr, Some(socket_opts)).await?;
            let fd = tcp_stream.as_connection_fd();

            debug!("connect to tls domain: {}", self.domain);
            let (write, read) = self
                .connector
                .connect(&self.domain, tcp_stream)
                .await
                .map_err(|e| {
                    IoError::new(
                        ErrorKind::ConnectionRefused,
                        format!("failed to connect: {}", e),
                    )
                })?
                .split_connection();
            Ok((write, read, fd))
        }

        fn new_domain(&self, domain: String) -> DomainConnector {
            let mut connector = self.clone();
            connector.domain = domain;
            Box::new(connector)
        }

        fn domain(&self) -> &str {
            &self.domain
        }

        fn clone_box(&self) -> DomainConnector {
            Box::new(self.clone())
        }
    }
}

pub use cert::*;

mod cert {
    use crate::net::certs::CertBuilder;
    use anyhow::{Context, Result};
    use native_tls::Certificate as NativeCertificate;
    use native_tls::Identity;
    use openssl::pkcs12::Pkcs12;
    use openssl::pkey::Private;

    pub type Certificate = openssl::x509::X509;
    pub type PrivateKey = openssl::pkey::PKey<Private>;

    pub struct X509PemBuilder(Vec<u8>);

    impl CertBuilder for X509PemBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl X509PemBuilder {
        pub fn build(self) -> Result<Certificate> {
            let cert = Certificate::from_pem(&self.0).context("invalid cert")?;
            Ok(cert)
        }

        pub fn build_native(self) -> Result<NativeCertificate> {
            NativeCertificate::from_pem(&self.0).context("invalid pem file")
        }
    }

    pub struct PrivateKeyBuilder(Vec<u8>);

    impl CertBuilder for PrivateKeyBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl PrivateKeyBuilder {
        pub fn build(self) -> Result<PrivateKey> {
            let key = PrivateKey::private_key_from_pem(&self.0).context("invalid key")?;
            Ok(key)
        }
    }

    const PASSWORD: &str = "test";

    pub struct IdentityBuilder(Vec<u8>);

    impl CertBuilder for IdentityBuilder {
        fn new(bytes: Vec<u8>) -> Self {
            Self(bytes)
        }
    }

    impl IdentityBuilder {
        /// load pk12 from x509 certs
        pub fn from_x509(x509: X509PemBuilder, key: PrivateKeyBuilder) -> Result<Self> {
            let server_key = key.build()?;
            let server_crt = x509.build()?;
            let p12 = Pkcs12::builder()
                .name("")
                .pkey(&server_key)
                .cert(&server_crt)
                .build2(PASSWORD)
                .context("Failed to create Pkcs12")?;

            let der = p12.to_der()?;
            Ok(Self(der))
        }

        pub fn build(self) -> Result<Identity> {
            Identity::from_pkcs12(&self.0, PASSWORD).context("Failed to load der")
        }
    }
}

pub use builder::*;

mod builder {
    use anyhow::{Context, Result};
    use native_tls::Identity;
    use native_tls::TlsAcceptor as NativeTlsAcceptor;

    use super::IdentityBuilder;
    use super::TlsAcceptor;
    use super::TlsConnector;
    use super::X509PemBuilder;

    pub struct ConnectorBuilder(TlsConnector);

    impl ConnectorBuilder {
        pub fn identity(builder: IdentityBuilder) -> Result<Self> {
            let identity = builder.build()?;
            let connector = TlsConnector::new().identity(identity);
            Ok(Self(connector))
        }

        pub fn anonymous() -> Self {
            let connector = TlsConnector::new()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true);
            Self(connector)
        }

        pub fn no_cert_verification(self) -> Self {
            let connector = self.0.danger_accept_invalid_certs(true);
            Self(connector)
        }

        pub fn danger_accept_invalid_hostnames(self) -> Self {
            let connector = self.0.danger_accept_invalid_hostnames(true);
            Self(connector)
        }

        pub fn use_sni(self, use_sni: bool) -> Self {
            let connector = self.0.use_sni(use_sni);
            Self(connector)
        }

        pub fn add_root_certificate(self, builder: X509PemBuilder) -> Result<Self> {
            let certificate = builder.build_native()?;
            let connector = self.0.add_root_certificate(certificate);
            Ok(Self(connector))
        }

        pub fn build(self) -> TlsConnector {
            self.0
        }
    }

    pub struct AcceptorBuilder(Identity);

    impl AcceptorBuilder {
        pub fn identity(builder: IdentityBuilder) -> Result<Self> {
            let identity = builder.build()?;
            Ok(Self(identity))
        }

        pub fn build(self) -> Result<TlsAcceptor> {
            let acceptor = NativeTlsAcceptor::new(self.0).context("invalid cert")?;
            Ok(acceptor.into())
        }
    }
}

#[cfg(test)]
mod test {

    use std::net::SocketAddr;
    use std::time;

    use anyhow::Result;
    use async_native_tls::TlsAcceptor;
    use async_native_tls::TlsConnector;
    use bytes::Buf;
    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_util::sink::SinkExt;
    use tokio_util::codec::BytesCodec;
    use tokio_util::codec::Framed;
    use tokio_util::compat::FuturesAsyncReadCompatExt;
    use tracing::debug;

    use crate::net::TcpListener;
    use crate::net::certs::CertBuilder;
    use crate::net::tcp_stream::stream;
    use crate::test_async;
    use crate::timer::sleep;

    use super::{
        AcceptorBuilder, ConnectorBuilder, IdentityBuilder, PrivateKeyBuilder, X509PemBuilder,
    };

    const CA_PATH: &str = "certs/test-certs/ca.crt";
    const SERVER_IDENTITY: &str = "certs/test-certs/server.pfx";
    const CLIENT_IDENTITY: &str = "certs/test-certs/client.pfx";
    const X509_SERVER: &str = "certs/test-certs/server.crt";
    const X509_SERVER_KEY: &str = "certs/test-certs/server.key";
    const X509_CLIENT: &str = "certs/test-certs/client.crt";
    const X509_CLIENT_KEY: &str = "certs/test-certs/client.key";

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }

    #[test_async]
    async fn test_native_tls_pk12() -> Result<()> {
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
    #[cfg(not(windows))]
    async fn test_native_tls_x509() -> Result<()> {
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

    async fn test_tls(port: u16, acceptor: TlsAcceptor, connector: TlsConnector) -> Result<()> {
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

            Ok(()) as Result<()>
        };

        let _ = zip(client_ft, server_ft).await;

        Ok(())
    }
}
