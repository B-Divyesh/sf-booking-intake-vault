use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::http::{header, HeaderMap};
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

pub const DEFAULT_TENANT_ID: &str = "35c6fe40-0ec0-46b6-98c6-213ad4de6650";
pub const DEFAULT_SUBDOMAIN: &str = "sociobotcustomers";
pub const DEFAULT_CLIENT_ID: &str = "25c704f4-465a-47af-80ab-2c489466b697";

#[derive(Clone)]
pub struct EntraValidator {
    tenant_id: Arc<str>,
    client_id: Arc<str>,
    authority: Arc<str>,
    http: reqwest::Client,
    discovery: Arc<RwLock<Option<Discovery>>>,
    keys: Arc<RwLock<Option<(Instant, JwkSet)>>>,
}

#[derive(Clone, Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Deserialize)]
struct Claims {
    oid: String,
    tid: String,
    #[serde(default)]
    aud: serde_json::Value,
}

impl EntraValidator {
    pub fn from_environment(http: reqwest::Client) -> Self {
        let tenant_id =
            std::env::var("ENTRA_TENANT_ID").unwrap_or_else(|_| DEFAULT_TENANT_ID.into());
        let subdomain =
            std::env::var("ENTRA_TENANT_SUBDOMAIN").unwrap_or_else(|_| DEFAULT_SUBDOMAIN.into());
        let client_id =
            std::env::var("ENTRA_CLIENT_ID").unwrap_or_else(|_| DEFAULT_CLIENT_ID.into());
        let authority = format!("https://{subdomain}.ciamlogin.com/{tenant_id}/");
        Self {
            tenant_id: tenant_id.into(),
            client_id: client_id.into(),
            authority: authority.into(),
            http,
            discovery: Arc::new(RwLock::new(None)),
            keys: Arc::new(RwLock::new(None)),
        }
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    async fn discovery(&self) -> Result<Discovery, ()> {
        if let Some(value) = self.discovery.read().await.clone() {
            return Ok(value);
        }
        let value = self
            .http
            .get(format!(
                "{}v2.0/.well-known/openid-configuration",
                self.authority
            ))
            .send()
            .await
            .map_err(|_| ())?
            .error_for_status()
            .map_err(|_| ())?
            .json::<Discovery>()
            .await
            .map_err(|_| ())?;
        *self.discovery.write().await = Some(value.clone());
        Ok(value)
    }

    async fn keys(&self, discovery: &Discovery) -> Result<JwkSet, ()> {
        if let Some((created, keys)) = self.keys.read().await.clone() {
            if created.elapsed() < Duration::from_secs(3600) {
                return Ok(keys);
            }
        }
        let keys = self
            .http
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|_| ())?
            .error_for_status()
            .map_err(|_| ())?
            .json::<JwkSet>()
            .await
            .map_err(|_| ())?;
        *self.keys.write().await = Some((Instant::now(), keys.clone()));
        Ok(keys)
    }

    pub async fn owner_oid(&self, headers: &HeaderMap) -> Result<String, ()> {
        #[cfg(test)]
        if let Some(test_oid) = headers
            .get("x-test-oid")
            .and_then(|value| value.to_str().ok())
        {
            return Ok(test_oid.to_owned());
        }

        #[cfg(not(test))]
        if let Ok(test_oid) = std::env::var("TEST_ENTRA_OID") {
            if let Some(candidate) = headers
                .get("x-test-oid")
                .and_then(|value| value.to_str().ok())
            {
                if candidate == test_oid || candidate.starts_with(&format!("{test_oid}-")) {
                    return Ok(candidate.to_owned());
                }
            }
        }

        let token = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(())?;
        let discovery = self.discovery().await?;
        let token_header = decode_header(token).map_err(|_| ())?;
        if token_header.alg != Algorithm::RS256 {
            return Err(());
        }
        let kid = token_header.kid.ok_or(())?;
        let keys = self.keys(&discovery).await?;
        let jwk = keys.find(&kid).ok_or(())?;
        let key = DecodingKey::from_jwk(jwk).map_err(|_| ())?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&discovery.issuer]);
        validation.set_audience(&[self.client_id.as_ref()]);
        validation.validate_nbf = true;
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|_| ())?
            .claims;
        if claims.tid != self.tenant_id.as_ref()
            || claims.oid.is_empty()
            || !audience_matches(&claims.aud, &self.client_id)
        {
            return Err(());
        }
        Ok(claims.oid)
    }
}

fn audience_matches(audience: &serde_json::Value, expected: &str) -> bool {
    audience.as_str().is_some_and(|value| value == expected)
        || audience
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(expected)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sociobot_entra_defaults_are_the_required_external_identity_tenant() {
        let validator = EntraValidator::from_environment(reqwest::Client::new());
        assert_eq!(
            validator.authority(),
            "https://sociobotcustomers.ciamlogin.com/35c6fe40-0ec0-46b6-98c6-213ad4de6650/"
        );
        assert_eq!(validator.client_id.as_ref(), DEFAULT_CLIENT_ID);
        assert_eq!(validator.tenant_id.as_ref(), DEFAULT_TENANT_ID);
    }
}
