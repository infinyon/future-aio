use std::{str::FromStr, sync::Arc};

use async_rustls::rustls::{OwnedTrustAnchor, RootCertStore};
use hyper::Uri;

use super::{
    async_std_compat::{self, CompatConnector},
    request::RequestBuilder,
};

type HyperClient = Arc<hyper::Client<CompatConnector, hyper::Body>>;

#[derive(Clone)]
pub struct Client {
    pub(crate) hyper: HyperClient,
}

impl Client {
    pub fn new() -> Self {
        ClientBuilder::default().build()
    }

    pub fn get(&self, uri: impl AsRef<str>) -> RequestBuilder {
        let req = http::request::Builder::new().uri(Uri::from_str(uri.as_ref()).unwrap());
        RequestBuilder::new(self.clone(), req)
    }
}

#[derive(Default)]
#[must_use]
pub struct ClientBuilder;

impl ClientBuilder {
    pub fn build(self) -> Client {
        let mut cert = RootCertStore::empty();
        cert.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
            OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));

        let tls = async_rustls::rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(cert)
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
