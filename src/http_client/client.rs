use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, Result};
use futures_rustls::rustls::{pki_types::TrustAnchor, RootCertStore};
use hyper::{Body, Uri};
use once_cell::sync::Lazy;

use super::{
    async_std_compat::{self, CompatConnector},
    request::RequestBuilder,
    USER_AGENT,
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
        TrustAnchor::from_subject_spki_name_constraints(ta.subject, ta.spki, ta.name_constraints)
    }));
    store
});

impl Default for Client {
    fn default() -> Self {
        let tls = futures_rustls::rustls::ClientConfig::builder().with_safe_defaults();

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

    pub async fn send<B: Into<hyper::Body>>(
        &self,
        req: http::Request<B>,
    ) -> Result<http::Response<Body>, anyhow::Error> {
        // convert http::Request into hyper::Request
        let (mut parts, body) = req.into_parts();
        let body: hyper::Body = body.into();
        if !parts.headers.contains_key(http::header::USER_AGENT) {
            let agent = http::header::HeaderValue::from_static(USER_AGENT);
            parts.headers.append(http::header::USER_AGENT, agent);
        }
        let req = hyper::Request::<hyper::Body>::from_parts(parts, body);
        self.hyper
            .request(req)
            .await
            .map_err(|err| anyhow!("request error: {err:?}"))
    }
}

#[cfg(feature = "__skip-http-client-cert-verification")]
mod no_verifier {

    pub use crate::rust_tls::fake_verifier::NoCertificateVerification;
}
