use openssl::pkey::{PKey, Private};
use openssl::x509::X509;

use super::error::Result;

#[derive(Debug)]
pub struct Certificate(pub X509);

impl Certificate {
    pub fn from_pem(bytes: &[u8]) -> Result<Self> {
        Ok(Self(X509::from_pem(bytes)?))
    }
    pub fn from_der(bytes: &[u8]) -> Result<Self> {
        Ok(Self(X509::from_der(bytes)?))
    }
    pub fn to_der(&self) -> Result<Vec<u8>> {
        Ok(self.0.to_der()?)
    }
}
pub struct PrivateKey(pub PKey<Private>);
