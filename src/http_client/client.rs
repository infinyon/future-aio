use std::{str::FromStr, sync::Arc};

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

static ROOT_CERT_STORE: Lazy<RootCertStore> = Lazy::new(|| {
    let mut cert = RootCertStore::empty();
    cert.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    cert
});

impl Default for Client {
    fn default() -> Self {
        let tls = async_rustls::rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(ROOT_CERT_STORE.to_owned())
            .with_no_client_auth();

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

    pub fn get(&self, uri: impl AsRef<str>) -> RequestBuilder {
        let req = http::request::Builder::new().uri(Uri::from_str(uri.as_ref()).unwrap());
        RequestBuilder::new(self.clone(), req)
    }
}
