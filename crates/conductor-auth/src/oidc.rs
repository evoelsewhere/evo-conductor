//! Generic OIDC authorization-code + PKCE — works for Microsoft Entra ID (Azure AD),
//! Google, and any standard OIDC provider.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use conductor_domain::{ConductorError, Result, SsoProvider};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

#[derive(Debug, Clone)]
pub struct OidcAuthRequest {
    pub authorization_url: String,
    pub state: String,
    pub code_verifier: String,
}

#[derive(Debug, Clone)]
pub struct OidcProfile {
    pub subject: String,
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    #[allow(dead_code)]
    jwks_uri: Option<String>,
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
    email: Option<String>,
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

async fn discover(issuer: &str) -> Result<OidcDiscovery> {
    let issuer = normalize_issuer(issuer);
    let url = format!("{issuer}/.well-known/openid-configuration");
    let resp = reqwest::Client::new()
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

    resp.json::<OidcDiscovery>()
        .await
        .map_err(|e| ConductorError::msg(format!("OIDC discovery parse failed: {e}")))
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

/// Start OIDC login — returns authorize URL + PKCE verifier (store verifier by state).
pub async fn begin_authorization(
    issuer_url: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
) -> Result<OidcAuthRequest> {
    let discovery = discover(issuer_url).await?;
    let (code_verifier, code_challenge) = pkce_pair();
    let state = random_state();

    let mut url = Url::parse(&discovery.authorization_endpoint)
        .map_err(|e| ConductorError::msg(format!("invalid authorization_endpoint: {e}")))?;

    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", client_id);
        q.append_pair("response_type", "code");
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair(
            "scope",
            &scopes.join(" "),
        );
        q.append_pair("state", &state);
        q.append_pair("code_challenge", &code_challenge);
        q.append_pair("code_challenge_method", "S256");
        // Microsoft Entra recommends this for SPA/web apps requesting id_token.
        q.append_pair("response_mode", "query");
    }

    Ok(OidcAuthRequest {
        authorization_url: url.to_string(),
        state,
        code_verifier,
    })
}

/// Exchange authorization code for an id_token and extract the user profile.
pub async fn exchange_code(
    issuer_url: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<OidcProfile> {
    let discovery = discover(issuer_url).await?;

    let client = reqwest::Client::new();
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

    let claims = decode_id_token_claims(&id_token)?;
    let email = claims
        .email
        .or(claims.preferred_username)
        .filter(|s| s.contains('@'))
        .ok_or_else(|| ConductorError::msg("OIDC id_token missing email claim"))?;

    let display_name = claims
        .name
        .or_else(|| match (claims.given_name, claims.family_name) {
            (Some(g), Some(f)) => Some(format!("{g} {f}")),
            (Some(g), None) => Some(g),
            _ => None,
        })
        .unwrap_or_else(|| email.clone());

    Ok(OidcProfile {
        subject: claims.sub,
        email,
        display_name,
    })
}

/// Decode JWT payload without signature verification.
/// Signature validation via JWKS can be added; Azure TLS + client secret exchange
/// already authenticates the token response channel for this base.
fn decode_id_token_claims(id_token: &str) -> Result<IdTokenClaims> {
    let mut parts = id_token.split('.');
    let _header = parts.next();
    let payload = parts
        .next()
        .ok_or_else(|| ConductorError::msg("malformed id_token"))?;

    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload))
        .map_err(|e| ConductorError::msg(format!("id_token payload decode failed: {e}")))?;

    serde_json::from_slice(&decoded)
        .map_err(|e| ConductorError::msg(format!("id_token claims parse failed: {e}")))
}
