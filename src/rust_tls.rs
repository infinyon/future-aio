use crate::net::TcpStream;

pub use futures_rustls::client::TlsStream as ClientTlsStream;
pub use futures_rustls::server::TlsStream as ServerTlsStream;
pub use futures_rustls::TlsAcceptor;
pub use futures_rustls::TlsConnector;

pub type DefaultServerTlsStream = ServerTlsStream<TcpStream>;
pub type DefaultClientTlsStream = ClientTlsStream<TcpStream>;

pub use builder::*;
pub use cert::*;
pub use connector::*;

mod split {

    use futures_util::AsyncReadExt;

    use super::*;
    use crate::net::{BoxReadConnection, BoxWriteConnection, SplitConnection};

    impl SplitConnection for DefaultClientTlsStream {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            let (read, write) = self.split();
            (Box::new(write), Box::new(read))
        }
    }

    impl SplitConnection for DefaultServerTlsStream {
        fn split_connection(self) -> (BoxWriteConnection, BoxReadConnection) {
            let (read, write) = self.split();
            (Box::new(write), Box::new(read))
        }
    }
}

mod cert {
    use std::fs::File;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::path::Path;

    use anyhow::{anyhow, Context, Result};
    use futures_rustls::rustls::pki_types::CertificateDer;
    use futures_rustls::rustls::pki_types::PrivateKeyDer;
    use futures_rustls::rustls::RootCertStore;
    use rustls_pemfile::certs;
    use rustls_pemfile::pkcs8_private_keys;

    pub fn load_certs<P: AsRef<Path>>(path: P) -> Result<Vec<CertificateDer<'static>>> {
        load_certs_from_reader(&mut BufReader::new(File::open(path)?))
    }

    pub fn load_certs_from_reader(rd: &mut dyn BufRead) -> Result<Vec<CertificateDer<'static>>> {
        certs(rd).map(|r| r.context("invalid cert")).collect()
    }

    /// Load the passed keys file
    pub fn load_keys<P: AsRef<Path>>(path: P) -> Result<Vec<PrivateKeyDer<'static>>> {
        load_keys_from_reader(&mut BufReader::new(File::open(path)?))
    }

    pub fn load_keys_from_reader(rd: &mut dyn BufRead) -> Result<Vec<PrivateKeyDer<'static>>> {
        pkcs8_private_keys(rd)
            .map(|r| r.map(|p| p.into()).context("invalid key"))
            .collect()
    }

    pub(crate) fn load_first_key<P: AsRef<Path>>(path: P) -> Result<PrivateKeyDer<'static>> {
        load_first_key_from_reader(&mut BufReader::new(File::open(path)?))
    }

    pub(crate) fn load_first_key_from_reader(
        rd: &mut dyn BufRead,
    ) -> Result<PrivateKeyDer<'static>> {
        let mut keys = load_keys_from_reader(rd)?;

        if keys.is_empty() {
            Err(anyhow!("no keys found"))
        } else {
            Ok(keys.remove(0))
        }
    }

    pub fn load_root_ca<P: AsRef<Path>>(path: P) -> Result<RootCertStore> {
        let certs = load_certs(path).map_err(|err| err.context("invalid ca crt"))?;

        let mut root_store = RootCertStore::empty();

        for cert in certs {
            root_store.add(cert).context("invalid ca crt")?;
        }

        Ok(root_store)
    }
}

mod connector {
    use std::io::Error as IoError;
    use std::io::ErrorKind;

    use async_trait::async_trait;
    use futures_rustls::rustls::pki_types::ServerName;
    use tracing::debug;

    use crate::net::{
        tcp_stream::stream, AsConnectionFd, BoxReadConnection, BoxWriteConnection, ConnectionFd,
        DomainConnector, SplitConnection, TcpDomainConnector,
    };

    use super::TlsConnector;

    pub type TlsError = IoError;

    /// connect as anonymous client
    #[derive(Clone)]
    pub struct TlsAnonymousConnector(TlsConnector);

    impl From<TlsConnector> for TlsAnonymousConnector {
        fn from(connector: TlsConnector) -> Self {
            Self(connector)
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

            let server_name = ServerName::try_from(domain).map_err(|err| {
                IoError::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid Dns Name: {}", err),
                )
            })?;

            let (write, read) = self
                .0
                .connect(server_name.to_owned(), tcp_stream)
                .await?
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

    #[derive(Clone)]
    pub struct TlsDomainConnector {
        domain: String,
        connector: TlsConnector,
    }

    impl TlsDomainConnector {
        pub fn new(connector: TlsConnector, domain: String) -> Self {
            Self { domain, connector }
        }
    }

    #[async_trait]
    impl TcpDomainConnector for TlsDomainConnector {
        async fn connect(
            &self,
            addr: &str,
        ) -> Result<(BoxWriteConnection, BoxReadConnection, ConnectionFd), IoError> {
            debug!("connect to tls addr: {}", addr);
            let tcp_stream = stream(addr).await?;
            let fd = tcp_stream.as_connection_fd();
            debug!("connect to tls domain: {}", self.domain);

            let server_name = ServerName::try_from(self.domain.as_str()).map_err(|err| {
                IoError::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid Dns Name: {}", err),
                )
            })?;

            let (write, read) = self
                .connector
                .connect(server_name.to_owned(), tcp_stream)
                .await?
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

mod builder {

    use std::io::Cursor;
    use std::path::Path;
    use std::sync::Arc;

    use futures_rustls::pki_types::UnixTime;
    use futures_rustls::rustls::client::danger::HandshakeSignatureValid;
    use futures_rustls::rustls::client::danger::ServerCertVerified;
    use futures_rustls::rustls::client::danger::ServerCertVerifier;
    use futures_rustls::rustls::client::WantsClientCert;
    use futures_rustls::rustls::pki_types::CertificateDer;
    use futures_rustls::rustls::pki_types::PrivateKeyDer;
    use futures_rustls::rustls::pki_types::ServerName;
    use futures_rustls::rustls::server::WantsServerCert;
    use futures_rustls::rustls::server::WebPkiClientVerifier;
    use futures_rustls::rustls::ClientConfig;
    use futures_rustls::rustls::ConfigBuilder;
    use futures_rustls::rustls::Error as TlsError;
    use futures_rustls::rustls::RootCertStore;
    use futures_rustls::rustls::ServerConfig;
    use futures_rustls::rustls::SignatureScheme;
    use futures_rustls::rustls::WantsVerifier;
    use futures_rustls::TlsAcceptor;
    use futures_rustls::TlsConnector;

    use anyhow::{Context, Result};
    use tracing::info;

    use super::load_root_ca;
    use super::{load_certs, load_first_key_from_reader};
    use super::{load_certs_from_reader, load_first_key};

    pub type ClientConfigBuilder<Stage> = ConfigBuilder<ClientConfig, Stage>;

    pub struct ConnectorBuilder;

    impl ConnectorBuilder {
        pub fn with_safe_defaults() -> ConnectorBuilderStage<WantsVerifier> {
            ConnectorBuilderStage(ClientConfig::builder())
        }
    }

    pub struct ConnectorBuilderStage<S>(ConfigBuilder<ClientConfig, S>);

    impl ConnectorBuilderStage<WantsVerifier> {
        pub fn load_ca_cert<P: AsRef<Path>>(
            self,
            path: P,
        ) -> Result<ConnectorBuilderStage<WantsClientCert>> {
            let certs = load_certs(path)?;
            self.with_root_certificates(certs)
        }

        pub fn load_ca_cert_from_bytes(
            self,
            buffer: &[u8],
        ) -> Result<ConnectorBuilderStage<WantsClientCert>> {
            let certs = load_certs_from_reader(&mut Cursor::new(buffer))?;
            self.with_root_certificates(certs)
        }

        pub fn no_cert_verification(self) -> ConnectorBuilderWithConfig {
            let config = self
                .0
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoCertificateVerification))
                .with_no_client_auth();

            ConnectorBuilderWithConfig(config)
        }

        fn with_root_certificates(
            self,
            certs: Vec<CertificateDer>,
        ) -> Result<ConnectorBuilderStage<WantsClientCert>> {
            let mut root_store = RootCertStore::empty();

            for cert in certs {
                root_store.add(cert).context("invalid ca crt")?;
            }

            Ok(ConnectorBuilderStage(
                self.0.with_root_certificates(root_store),
            ))
        }
    }

    impl ConnectorBuilderStage<WantsClientCert> {
        pub fn load_client_certs<P: AsRef<Path>>(
            self,
            cert_path: P,
            key_path: P,
        ) -> Result<ConnectorBuilderWithConfig> {
            let certs = load_certs(cert_path)?;
            let key = load_first_key(key_path)?;
            self.with_single_cert(certs, key)
        }

        pub fn load_client_certs_from_bytes(
            self,
            cert_buf: &[u8],
            key_buf: &[u8],
        ) -> Result<ConnectorBuilderWithConfig> {
            let certs = load_certs_from_reader(&mut Cursor::new(cert_buf))?;
            let key = load_first_key_from_reader(&mut Cursor::new(key_buf))?;
            self.with_single_cert(certs, key)
        }

        pub fn no_client_auth(self) -> ConnectorBuilderWithConfig {
            ConnectorBuilderWithConfig(self.0.with_no_client_auth())
        }

        fn with_single_cert(
            self,
            certs: Vec<CertificateDer<'static>>,
            key: PrivateKeyDer<'static>,
        ) -> Result<ConnectorBuilderWithConfig> {
            let config = self
                .0
                .with_client_auth_cert(certs, key)
                .context("invalid cert")?;

            Ok(ConnectorBuilderWithConfig(config))
        }
    }

    pub struct ConnectorBuilderWithConfig(ClientConfig);

    impl ConnectorBuilderWithConfig {
        pub fn build(self) -> TlsConnector {
            Arc::new(self.0).into()
        }
    }

    pub struct AcceptorBuilder;

    impl AcceptorBuilder {
        pub fn with_safe_defaults() -> AcceptorBuilderStage<WantsVerifier> {
            AcceptorBuilderStage(ServerConfig::builder())
        }
    }

    pub struct AcceptorBuilderStage<S>(ConfigBuilder<ServerConfig, S>);

    impl AcceptorBuilderStage<WantsVerifier> {
        /// Require no client authentication.
        pub fn no_client_authentication(self) -> AcceptorBuilderStage<WantsServerCert> {
            AcceptorBuilderStage(self.0.with_no_client_auth())
        }

        /// Require client authentication. Must pass CA root path.
        pub fn client_authenticate<P: AsRef<Path>>(
            self,
            path: P,
        ) -> Result<AcceptorBuilderStage<WantsServerCert>> {
            let root_store = load_root_ca(path)?;

            let client_verifier = WebPkiClientVerifier::builder(root_store.into())
                .build()
                .context("invalid verifier")?;

            Ok(AcceptorBuilderStage(
                self.0.with_client_cert_verifier(client_verifier),
            ))
        }
    }

    impl AcceptorBuilderStage<WantsServerCert> {
        pub fn load_server_certs(
            self,
            cert_path: impl AsRef<Path>,
            key_path: impl AsRef<Path>,
        ) -> Result<AcceptorBuilderWithConfig> {
            let certs = load_certs(cert_path)?;
            let key = load_first_key(key_path)?;

            let config = self
                .0
                .with_single_cert(certs, key)
                .context("invalid cert")?;

            Ok(AcceptorBuilderWithConfig(config))
        }
    }

    pub struct AcceptorBuilderWithConfig(ServerConfig);

    impl AcceptorBuilderWithConfig {
        pub fn build(self) -> TlsAcceptor {
            TlsAcceptor::from(Arc::new(self.0))
        }
    }

    #[derive(Debug)]
    struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer,
            _intermediates: &[CertificateDer],
            _server_name: &ServerName,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, TlsError> {
            info!("ignoring server cert");
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &futures_rustls::rustls::DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, TlsError> {
            info!("ignoring server cert");
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &futures_rustls::rustls::DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, TlsError> {
            info!("ignoring server cert");
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            let provider = futures_rustls::rustls::crypto::aws_lc_rs::default_provider();
            provider
                .signature_verification_algorithms
                .supported_schemes()
        }
    }
}

#[cfg(test)]
mod test {

    use std::net::SocketAddr;
    use std::time;

    use bytes::BufMut;
    use bytes::Bytes;
    use bytes::BytesMut;
    use futures_lite::future::zip;
    use futures_lite::stream::StreamExt;
    use futures_rustls::TlsAcceptor;
    use futures_rustls::TlsConnector;
    use futures_util::sink::SinkExt;
    use tokio_util::codec::BytesCodec;
    use tokio_util::codec::Framed;
    use tokio_util::compat::FuturesAsyncReadCompatExt;
    use tracing::debug;

    use fluvio_future::net::tcp_stream::stream;
    use fluvio_future::net::TcpListener;
    use fluvio_future::test_async;
    use fluvio_future::timer::sleep;

    use anyhow::Result;

    use super::{AcceptorBuilder, ConnectorBuilder};

    const CA_PATH: &str = "certs/test-certs/ca.crt";
    const ITER: u16 = 10;

    fn to_bytes(bytes: Vec<u8>) -> Bytes {
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.freeze()
    }

    #[test_async(ignore)]
    async fn test_rust_tls_all() -> Result<()> {
        test_rustls(
            AcceptorBuilder::with_safe_defaults()
                .no_client_authentication()
                .load_server_certs("certs/test-certs/server.crt", "certs/test-certs/server.key")?
                .build(),
            ConnectorBuilder::with_safe_defaults()
                .no_cert_verification()
                .build(),
        )
        .await
        .expect("no client cert test failed");

        // test client authentication

        test_rustls(
            AcceptorBuilder::with_safe_defaults()
                .client_authenticate(CA_PATH)?
                .load_server_certs("certs/test-certs/server.crt", "certs/test-certs/server.key")?
                .build(),
            ConnectorBuilder::with_safe_defaults()
                .load_ca_cert(CA_PATH)?
                .load_client_certs("certs/test-certs/client.crt", "certs/test-certs/client.key")?
                .build(),
        )
        .await
        .expect("client cert test fail");

        Ok(())
    }

    async fn test_rustls(acceptor: TlsAcceptor, connector: TlsConnector) -> Result<()> {
        let addr = "127.0.0.1:19998".parse::<SocketAddr>().expect("parse");

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
            let tls_stream = acceptor.accept(tcp_stream).await.expect("accept");

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
                .connect("localhost".try_into().expect("domain"), tcp_stream)
                .await
                .expect("tls failed");
            let all_stream = Box::new(tls_stream);
            let mut framed = Framed::new(all_stream.compat(), BytesCodec::new());
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
}
