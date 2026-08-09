//! Generic OIDC authorization-code + PKCE — works for Microsoft Entra ID,
//! Google, and standard OpenID Connect providers.

use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use conductor_domain::{ConductorError, Result, SsoProvider};
use jsonwebtoken::{
    decode, decode_header, jwk::AlgorithmParameters, jwk::JwkSet, Algorithm, DecodingKey,
    Validation,
};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Debug, Clone)]
pub struct OidcAuthRequest {
    pub authorization_url: String,
    pub state: String,
    pub code_verifier: String,
    pub nonce: String,
}

#[derive(Debug, Clone)]
pub struct OidcProfile {
    pub subject: String,
    pub issuer: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
    #[allow(dead_code)]
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
    iss: String,
    #[allow(dead_code)]
    exp: i64,
    nonce: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    preferred_username: Option<String>,
    name: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
}

/// Normalize issuer (strip trailing slash). For Azure AD, issuer should look like:
/// `https://login.microsoftonline.com/{tenant}/v2.0`
pub fn normalize_issuer(issuer: &str) -> String {
    issuer.trim().trim_end_matches('/').to_string()
}

pub fn default_scopes(provider: SsoProvider) -> Vec<String> {
    match provider {
        SsoProvider::AzureAd => vec![
            "openid".into(),
            "profile".into(),
            "email".into(),
            "offline_access".into(),
        ],
        _ => vec!["openid".into(), "profile".into(), "email".into()],
    }
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| ConductorError::msg(format!("OIDC HTTP client failed: {e}")))
}

fn validate_endpoint(value: &str, label: &str) -> Result<Url> {
    let url = Url::parse(value)
        .map_err(|_| ConductorError::msg(format!("OIDC {label} is not a valid URL")))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(ConductorError::msg(format!(
            "OIDC {label} must use HTTPS (HTTP is allowed only for loopback development)"
        )));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ConductorError::msg(format!(
            "OIDC {label} must not contain embedded credentials"
        )));
    }
    Ok(url)
}

pub fn validate_oidc_url(value: &str, label: &str) -> Result<()> {
    validate_endpoint(value, label).map(|_| ())
}

pub fn validate_oidc_redirect_uri(value: &str) -> Result<()> {
    let url = validate_endpoint(value, "redirect URI")?;
    if !url.path().ends_with("/api/auth/sso/callback")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ConductorError::msg(
            "OIDC redirect URI must end with /api/auth/sso/callback and have no query or fragment",
        ));
    }
    Ok(())
}

async fn discover(issuer: &str) -> Result<OidcDiscovery> {
    let issuer = normalize_issuer(issuer);
    validate_endpoint(&issuer, "issuer")?;
    let url = format!("{issuer}/.well-known/openid-configuration");
    let resp = http_client()?
        .get(&url)
        .send()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC discovery failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ConductorError::msg(format!(
            "OIDC discovery HTTP {}: {url}",
            resp.status()
        )));
    }

    let discovery = resp
        .json::<OidcDiscovery>()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC discovery parse failed: {e}")))?;
    if normalize_issuer(&discovery.issuer) != issuer {
        return Err(ConductorError::msg(
            "OIDC discovery issuer does not match the configured issuer",
        ));
    }
    validate_endpoint(&discovery.authorization_endpoint, "authorization endpoint")?;
    validate_endpoint(&discovery.token_endpoint, "token endpoint")?;
    validate_endpoint(&discovery.jwks_uri, "JWKS endpoint")?;
    Ok(discovery)
}

fn pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn random_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn random_nonce() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Start OIDC login and return the authorize URL plus values kept server-side.
pub async fn begin_authorization(
    issuer_url: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
) -> Result<OidcAuthRequest> {
    let discovery = discover(issuer_url).await?;
    let (code_verifier, code_challenge) = pkce_pair();
    let state = random_state();
    let nonce = random_nonce();

    let mut url = Url::parse(&discovery.authorization_endpoint)
        .map_err(|e| ConductorError::msg(format!("invalid authorization_endpoint: {e}")))?;

    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", client_id);
        q.append_pair("response_type", "code");
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair("scope", &scopes.join(" "));
        q.append_pair("state", &state);
        q.append_pair("nonce", &nonce);
        q.append_pair("code_challenge", &code_challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("response_mode", "query");
    }

    Ok(OidcAuthRequest {
        authorization_url: url.to_string(),
        state,
        code_verifier,
        nonce,
    })
}

/// Exchange the authorization code and cryptographically validate the id_token.
pub async fn exchange_code(
    issuer_url: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
    expected_nonce: &str,
) -> Result<OidcProfile> {
    let discovery = discover(issuer_url).await?;
    let client = http_client()?;
    let resp = client
        .post(&discovery.token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC token request failed: {e}")))?;

    let status = resp.status();
    let body = resp
        .json::<TokenResponse>()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC token response parse failed: {e}")))?;

    if !status.is_success() {
        let detail = body
            .error_description
            .or(body.error)
            .unwrap_or_else(|| status.to_string());
        return Err(ConductorError::msg(format!("OIDC token error: {detail}")));
    }

    let id_token = body
        .id_token
        .ok_or_else(|| ConductorError::msg("OIDC response missing id_token"))?;
    let claims = verify_id_token(&client, &discovery, client_id, &id_token, expected_nonce).await?;
    if claims.email_verified == Some(false) {
        return Err(ConductorError::msg("OIDC email claim is not verified"));
    }
    let email = claims
        .email
        .or(claims.preferred_username)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| value.contains('@'))
        .ok_or_else(|| ConductorError::msg("OIDC id_token missing email claim"))?;

    let display_name = claims
        .name
        .or_else(|| match (claims.given_name, claims.family_name) {
            (Some(g), Some(f)) => Some(format!("{g} {f}")),
            (Some(g), None) => Some(g),
            _ => None,
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| email.clone());

    Ok(OidcProfile {
        subject: claims.sub,
        issuer: claims.iss,
        email,
        display_name,
    })
}

async fn verify_id_token(
    client: &reqwest::Client,
    discovery: &OidcDiscovery,
    client_id: &str,
    id_token: &str,
    expected_nonce: &str,
) -> Result<IdTokenClaims> {
    let header = decode_header(id_token)
        .map_err(|_| ConductorError::msg("OIDC id_token has an invalid header"))?;
    if !matches!(
        header.alg,
        Algorithm::RS256
            | Algorithm::RS384
            | Algorithm::RS512
            | Algorithm::PS256
            | Algorithm::PS384
            | Algorithm::PS512
            | Algorithm::ES256
            | Algorithm::ES384
            | Algorithm::EdDSA
    ) {
        return Err(ConductorError::msg(
            "OIDC id_token uses an unsupported signing algorithm",
        ));
    }

    let kid = header
        .kid
        .as_deref()
        .ok_or_else(|| ConductorError::msg("OIDC id_token is missing kid"))?;
    let response = client
        .get(&discovery.jwks_uri)
        .send()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC JWKS request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(ConductorError::msg(format!(
            "OIDC JWKS HTTP {}",
            response.status()
        )));
    }
    let jwks = response
        .json::<JwkSet>()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC JWKS parse failed: {e}")))?;
    let jwk = jwks
        .find(kid)
        .ok_or_else(|| ConductorError::msg("OIDC signing key was not found"))?;
    if matches!(&jwk.algorithm, AlgorithmParameters::OctetKey(_)) {
        return Err(ConductorError::msg(
            "OIDC symmetric signing keys are not accepted",
        ));
    }

    let decoding_key = DecodingKey::from_jwk(jwk)
        .map_err(|_| ConductorError::msg("OIDC signing key is invalid"))?;
    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&[discovery.issuer.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.leeway = 60;

    let claims = decode::<IdTokenClaims>(id_token, &decoding_key, &validation)
        .map_err(|_| ConductorError::msg("OIDC id_token validation failed"))?
        .claims;
    if claims.nonce.as_deref() != Some(expected_nonce) {
        return Err(ConductorError::msg("OIDC nonce validation failed"));
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_issuer_without_changing_path() {
        assert_eq!(
            normalize_issuer(" https://login.example.test/tenant/v2.0/ "),
            "https://login.example.test/tenant/v2.0"
        );
    }

    #[test]
    fn rejects_plain_http_identity_providers() {
        assert!(validate_endpoint("http://idp.example.test", "issuer").is_err());
        assert!(validate_endpoint("http://127.0.0.1:5556", "issuer").is_ok());
    }
}
