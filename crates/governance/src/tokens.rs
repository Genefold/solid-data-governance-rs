//! DPoP-bound capability tokens.
//!
//! Phase 0 implementation: the issuer signs the [`CapabilityClaims`] as
//! a JWT using HMAC-SHA-256. The `cnf` claim carries the DPoP key
//! confirmation (`{"jkt": "<sha256-thumbprint>"}`), as defined by
//! RFC 9449. Verification of the DPoP proof itself happens in the
//! request middleware (Phase 1); for Phase 0 the binding is recorded
//! in the token and surfaced in audit events.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::policy::Tier;

/// JWT-shaped claim set bound to a single dataset request capability.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityClaims {
    /// JWT subject — opaque token id.
    pub sub: String,
    /// Solid WebID of the principal.
    pub webid: String,
    /// Dataset id (e.g. `"org-a/bert-v2"`).
    pub dataset_id: String,
    /// Access tier this capability grants.
    pub access_tier: String,
    /// Optional byte cap for evaluation tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_cap: Option<u64>,
    /// Expiration time (Unix seconds).
    pub exp: usize,
    /// Issued-at time (Unix seconds).
    pub iat: usize,
    /// DPoP key confirmation: `{"jkt": "<thumbprint>"}`.
    pub cnf: serde_json::Value,
}

impl CapabilityClaims {
    pub fn new(
        webid: impl Into<String>,
        dataset_id: impl Into<String>,
        tier: Tier,
        ttl_seconds: u64,
        byte_cap: Option<u64>,
        dpop_jkt: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp() as usize;
        Self {
            sub: uuid::Uuid::new_v4().to_string(),
            webid: webid.into(),
            dataset_id: dataset_id.into(),
            access_tier: tier.as_str().to_owned(),
            byte_cap,
            exp: now + ttl_seconds as usize,
            iat: now,
            cnf: serde_json::json!({ "jkt": dpop_jkt.into() }),
        }
    }
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("jwt error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

/// Symmetric-key JWT issuer + verifier.
///
/// Phase 0 uses HMAC-SHA-256 because the deployment surface is a single
/// pod-server. Phase 2 will swap this for an asymmetric KMS-backed
/// signer when multi-tenant hosted operations begin.
#[derive(Clone)]
pub struct TokenIssuer {
    secret: Vec<u8>,
}

impl TokenIssuer {
    /// Build an issuer from a raw secret. Use a randomly generated
    /// 32-byte secret in production; tests may use a fixed value.
    pub fn new(secret: Vec<u8>) -> Self {
        Self { secret }
    }

    /// Generate a fresh random 32-byte secret.
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        // Cheap, dependency-light source of randomness: hash of the
        // current monotonic + system time + uuid.
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(uuid::Uuid::new_v4().as_bytes());
        h.update(
            chrono::Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or(0)
                .to_le_bytes(),
        );
        let digest = h.finalize();
        bytes.copy_from_slice(&digest);
        Self::new(bytes.to_vec())
    }

    /// Sign a claim set and return the compact JWT string.
    pub fn sign(&self, claims: &CapabilityClaims) -> Result<String, TokenError> {
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
        let key = jsonwebtoken::EncodingKey::from_secret(&self.secret);
        Ok(jsonwebtoken::encode(&header, claims, &key)?)
    }

    /// Verify a JWT and return the claim set if it is currently valid.
    pub fn verify(&self, token: &str) -> Result<CapabilityClaims, TokenError> {
        let key = jsonwebtoken::DecodingKey::from_secret(&self.secret);
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.validate_exp = true;
        // Disable audience/issuer checks since we don't set them yet.
        validation.required_spec_claims.clear();
        validation.required_spec_claims.insert("exp".to_owned());
        let data = jsonwebtoken::decode::<CapabilityClaims>(token, &key, &validation)?;
        Ok(data.claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrip() {
        let issuer = TokenIssuer::new(b"phase-0-test-secret".to_vec());
        let claims = CapabilityClaims::new(
            "https://alice.example/#me",
            "org-a/bert-v2",
            Tier::Evaluation,
            3600,
            Some(20_000_000_000),
            "abcd1234",
        );
        let token = issuer.sign(&claims).unwrap();
        let parsed = issuer.verify(&token).unwrap();
        assert_eq!(parsed.dataset_id, "org-a/bert-v2");
        assert_eq!(parsed.access_tier, "evaluation");
        assert_eq!(parsed.byte_cap, Some(20_000_000_000));
        assert_eq!(parsed.cnf["jkt"], "abcd1234");
    }
}
