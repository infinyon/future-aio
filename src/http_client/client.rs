use std::{str::FromStr, sync::Arc};

use anyhow::Result;
use async_rustls::rustls::{OwnedTrustAnchor, RootCertStore};
use hyper::Uri;
use once_cell::sync::Lazy;

use super::{
    async_std_compat::{self, CompatConnector},
    request::RequestBuilder,
};

type HyperClient = Arc<hyper::Client<CompatConnector, hyper::Body>>;

#[derive(Clone)]
pub struct Client {
    pub(crate) hyper: HyperClient,
}

#[cfg_attr(feature = "__skip-http-client-cert-verification", allow(dead_code))]
static ROOT_CERT_STORE: Lazy<RootCertStore> = Lazy::new(|| {
    let mut store = RootCertStore::empty();
    store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    store
});

impl Default for Client {
    fn default() -> Self {
        let tls = async_rustls::rustls::ClientConfig::builder().with_safe_defaults();

        #[cfg(not(feature = "__skip-http-client-cert-verification"))]
        let tls = tls.with_root_certificates(ROOT_CERT_STORE.to_owned());

        #[cfg(feature = "__skip-http-client-cert-verification")]
        let tls =
            tls.with_custom_certificate_verifier(Arc::new(no_verifier::NoCertificateVerification));

        let tls = tls.with_no_client_auth();

        let https = async_std_compat::CompatConnector::new(tls);

        let client: hyper::Client<_, hyper::Body> = hyper::Client::builder()
            .executor(async_std_compat::CompatExecutor)
            .build(https);

        Client {
            hyper: Arc::new(client),
        }
    }
}

impl Client {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, uri: impl AsRef<str>) -> Result<RequestBuilder> {
        let uri = Uri::from_str(uri.as_ref())?;
        let req = http::request::Builder::new().uri(uri);
        Ok(RequestBuilder::new(self.clone(), req))
    }
}

#[cfg(feature = "__skip-http-client-cert-verification")]
mod no_verifier {
    use std::time::SystemTime;

    use async_rustls::rustls::{
        client::{ServerCertVerified, ServerCertVerifier},
        Certificate, Error, ServerName,
    };

    pub struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &Certificate,
            _intermediates: &[Certificate],
            _server_name: &ServerName,
            _scts: &mut dyn Iterator<Item = &[u8]>,
            _ocsp_response: &[u8],
            _now: SystemTime,
        ) -> Result<ServerCertVerified, Error> {
            Ok(ServerCertVerified::assertion())
        }
    }
}
