use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri_plugin_shell::ShellExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::{timeout, Duration};

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcConfig {
    pub provider_name: String,
    pub client_id: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    /// Defaults to ["openid", "email", "profile"] when empty.
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcProfile {
    /// Subject — unique user identifier from the IdP.
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    /// Full, raw claims payload from the ID token.
    pub raw_claims: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct OidcFlowResult {
    pub profile: OidcProfile,
    pub access_token: String,
    pub id_token: String,
    /// Ephemeral port that was used for the redirect listener.
    pub callback_port: u16,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct AuthState {
    pub configs: Arc<RwLock<Vec<OidcConfig>>>,
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            configs: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

// ── PKCE helpers ─────────────────────────────────────────────────────────

/// Generate a PKCE code verifier (43-128 char base64url string) and its
/// SHA-256 S256 challenge.  Returns `(verifier, challenge)`.
pub fn generate_pkce_pair() -> (String, String) {
    // 32 random bytes → 43 base64url characters (no padding), well within spec.
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = URL_SAFE_NO_PAD.encode(digest);

    (verifier, challenge)
}

// ── Port discovery ────────────────────────────────────────────────────────

/// Find a free TCP port in the range 12300–12399.
pub async fn find_free_port() -> Result<u16, String> {
    for port in 12300u16..=12399 {
        if TcpListener::bind(("127.0.0.1", port)).await.is_ok() {
            return Ok(port);
        }
    }
    Err("No free port available in range 12300–12399".to_string())
}

// ── Authorization URL ─────────────────────────────────────────────────────

/// Build the authorization URL with PKCE and a random `state` parameter.
pub fn build_auth_url(
    config: &OidcConfig,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let scopes = if config.scopes.is_empty() {
        "openid email profile".to_string()
    } else {
        config.scopes.join(" ")
    };

    let encoded_redirect = url_encode(redirect_uri);
    let encoded_scopes = url_encode(&scopes);
    let encoded_state = url_encode(state);
    let encoded_challenge = url_encode(code_challenge);

    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        config.authorization_endpoint,
        config.client_id,
        encoded_redirect,
        encoded_scopes,
        encoded_state,
        encoded_challenge,
    )
}

/// Minimal percent-encoder for URL query values (encodes everything except
/// unreserved characters as per RFC 3986).
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ── Callback listener ─────────────────────────────────────────────────────

/// Spin up a one-shot HTTP server on `port` that captures the callback path
/// once and returns it.  Times out after 120 seconds.
///
/// Returns the raw path string, e.g. `/callback?code=abc&state=xyz`.
pub async fn wait_for_callback(port: u16) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("Failed to bind port {port}: {e}"))?;

    let accept_fut = async {
        let (mut stream, _peer) = listener
            .accept()
            .await
            .map_err(|e| format!("Accept error: {e}"))?;

        // Read the HTTP request (up to 8 KiB is more than enough for a callback).
        let mut buf = vec![0u8; 8192];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| format!("Read error: {e}"))?;
        let request = String::from_utf8_lossy(&buf[..n]);

        // Extract the path from "GET /callback?... HTTP/1.1"
        let path = request
            .lines()
            .next()
            .and_then(|line| {
                let mut parts = line.splitn(3, ' ');
                parts.next(); // method
                parts.next() // path
            })
            .map(str::to_owned)
            .ok_or_else(|| "Could not parse HTTP request line".to_string())?;

        // Send a minimal "you can close this tab" response.
        let body = "<html><body><h2>Authentication successful &mdash; you can close this tab.</h2></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body,
        );
        // Best-effort write; ignore flush errors — the browser already has the code.
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;

        Ok::<String, String>(path)
    };

    timeout(Duration::from_secs(120), accept_fut)
        .await
        .map_err(|_| "OIDC callback timed out after 120 seconds".to_string())?
}

// ── Token exchange ────────────────────────────────────────────────────────

/// Exchange an authorization code for tokens via a form-encoded POST.
/// Returns the raw JSON token response from the IdP.
///
/// NOTE: This implementation uses a blocking `std::net::TcpStream` because
/// writing a correct HTTP/1.1 chunked-response parser on top of Tokio's
/// `AsyncRead` for a single use-site adds significant complexity with no
/// practical benefit.  The call is wrapped in `spawn_blocking` so it does
/// not block the async runtime.
pub async fn exchange_code_for_tokens(
    token_endpoint: &str,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<serde_json::Value, String> {
    let body = format!(
        "grant_type=authorization_code&client_id={}&code={}&redirect_uri={}&code_verifier={}",
        url_encode(client_id),
        url_encode(code),
        url_encode(redirect_uri),
        url_encode(code_verifier),
    );

    // Parse the token endpoint URL into host + path so we can open a raw TCP connection.
    let (host, port, path) = parse_https_url(token_endpoint)?;
    let addr = format!("{host}:{port}");

    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         Content-Type: application/x-www-form-urlencoded\r\n\
         Content-Length: {len}\r\n\
         Accept: application/json\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len(),
    );

    // Perform the blocking TCP write/read on a thread-pool thread.
    let response_body = tokio::task::spawn_blocking(move || {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(&addr)
            .map_err(|e| format!("TCP connect to {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("TCP write: {e}"))?;

        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| format!("TCP read: {e}"))?;
        Ok::<Vec<u8>, String>(raw)
    })
    .await
    .map_err(|e| format!("spawn_blocking error: {e}"))??;

    // Split HTTP headers from body at the double CRLF.
    let response_str = String::from_utf8_lossy(&response_body);
    let body_start = response_str
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .ok_or_else(|| "Malformed HTTP response: no header/body separator".to_string())?;
    let json_body = &response_body[body_start..];

    serde_json::from_slice(json_body)
        .map_err(|e| format!("Token endpoint returned non-JSON body: {e}"))
}

/// Parse an `https://host[:port]/path` URL into `(host, port, path)`.
fn parse_https_url(url: &str) -> Result<(String, u16, String), String> {
    let after_scheme = if let Some(s) = url.strip_prefix("https://") {
        s
    } else if let Some(s) = url.strip_prefix("http://") {
        s
    } else {
        return Err(format!("Unsupported scheme in URL: {url}"));
    };

    let (authority, path) = match after_scheme.find('/') {
        Some(i) => (&after_scheme[..i], after_scheme[i..].to_owned()),
        None => (after_scheme, "/".to_owned()),
    };

    let (host, port) = if let Some(colon) = authority.rfind(':') {
        let p = authority[colon + 1..]
            .parse::<u16>()
            .map_err(|_| format!("Invalid port in URL: {url}"))?;
        (authority[..colon].to_owned(), p)
    } else if url.starts_with("https://") {
        (authority.to_owned(), 443u16)
    } else {
        (authority.to_owned(), 80u16)
    };

    Ok((host, port, path))
}

// ── ID token parsing ──────────────────────────────────────────────────────

/// Parse a JWT ID token — base64url-decode the payload segment and return
/// the claims as a `serde_json::Value`.
///
/// **Signature verification is intentionally skipped.**  Full verification
/// requires fetching the IdP's JWKS endpoint, validating the `alg`, and
/// verifying the RSA/EC signature — out of scope for Phase 3.  Applications
/// that require cryptographic token verification should use a dedicated OIDC
/// library such as `openidconnect`.
pub fn parse_id_token_claims(id_token: &str) -> Result<serde_json::Value, String> {
    let parts: Vec<&str> = id_token.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err(format!(
            "Invalid JWT: expected 3 dot-separated segments, found {}",
            parts.len()
        ));
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .or_else(|_| {
            // Some implementations emit standard base64 (with padding); try that too.
            use base64::{engine::general_purpose::STANDARD, Engine as _};
            STANDARD.decode(parts[1])
        })
        .map_err(|e| format!("Failed to base64url-decode JWT payload: {e}"))?;

    serde_json::from_slice(&payload_bytes)
        .map_err(|e| format!("JWT payload is not valid JSON: {e}"))
}

/// Extract an `OidcProfile` from a parsed claims `Value`.
fn profile_from_claims(claims: serde_json::Value) -> Result<OidcProfile, String> {
    let sub = claims
        .get("sub")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "ID token is missing required 'sub' claim".to_string())?
        .to_owned();

    Ok(OidcProfile {
        sub,
        email: claims
            .get("email")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        name: claims
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        picture: claims
            .get("picture")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        raw_claims: claims,
    })
}

/// Generate a cryptographically random opaque `state` parameter (used to
/// prevent CSRF on the redirect).
fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Extract a named query parameter from a raw HTTP request path such as
/// `/callback?code=abc&state=xyz`.
fn extract_query_param(path: &str, key: &str) -> Option<String> {
    let query = path.splitn(2, '?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

/// Very small percent-decoder (handles `%XX` and `+` → space).
fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        } else if bytes[i] == b'+' {
            out.push(' ');
            i += 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

// ── Tauri commands ────────────────────────────────────────────────────────

/// Run the full OIDC Authorization Code + PKCE flow:
///
/// 1. Find a free ephemeral port in 12300–12399.
/// 2. Build the authorization URL.
/// 3. Open the system browser.
/// 4. Wait for the browser redirect to `http://127.0.0.1:{port}/callback`.
/// 5. Exchange the authorization code for tokens.
/// 6. Parse the ID token and return the user profile.
#[tauri::command]
pub async fn auth_oidc_begin(
    config: OidcConfig,
    app: tauri::AppHandle,
) -> Result<OidcFlowResult, String> {
    // Step 1 — ephemeral port
    let port = find_free_port().await?;
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Step 2 — PKCE + state
    let (verifier, challenge) = generate_pkce_pair();
    let state = generate_state();
    let url = build_auth_url(&config, &redirect_uri, &challenge, &state);

    // Step 3 — open browser
    tauri_plugin_shell::open(&app.shell(), &url, None)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    // Step 4 — wait for redirect (120 s timeout)
    let callback_path = wait_for_callback(port).await?;

    // Verify the state parameter to prevent CSRF.
    let returned_state = extract_query_param(&callback_path, "state")
        .ok_or_else(|| "OIDC callback missing 'state' parameter".to_string())?;
    if returned_state != state {
        return Err(format!(
            "OIDC state mismatch: expected '{state}', got '{returned_state}'"
        ));
    }

    let code = extract_query_param(&callback_path, "code")
        .ok_or_else(|| {
            // Surface IdP errors helpfully.
            let err = extract_query_param(&callback_path, "error")
                .unwrap_or_else(|| "unknown".to_string());
            let desc = extract_query_param(&callback_path, "error_description")
                .unwrap_or_default();
            format!("OIDC error from IdP: {err} — {desc}")
        })?;

    // Step 5 — exchange code for tokens
    let token_response =
        exchange_code_for_tokens(&config.token_endpoint, &config.client_id, &code, &redirect_uri, &verifier).await?;

    let access_token = token_response
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token response missing 'access_token'".to_string())?
        .to_owned();

    let id_token = token_response
        .get("id_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Token response missing 'id_token'".to_string())?
        .to_owned();

    // Step 6 — parse ID token claims → profile
    let claims = parse_id_token_claims(&id_token)?;
    let profile = profile_from_claims(claims)?;

    Ok(OidcFlowResult {
        profile,
        access_token,
        id_token,
        callback_port: port,
    })
}

/// Persist a new (or updated) OIDC provider configuration.
/// Replaces an existing entry with the same `provider_name`.
#[tauri::command]
pub fn auth_save_oidc_config(
    config: OidcConfig,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let mut configs = state
        .configs
        .try_write()
        .map_err(|_| "Auth state lock poisoned".to_string())?;

    if let Some(existing) = configs
        .iter_mut()
        .find(|c| c.provider_name == config.provider_name)
    {
        *existing = config;
    } else {
        configs.push(config);
    }
    Ok(())
}

/// Return all saved OIDC provider configurations.
#[tauri::command]
pub fn auth_list_oidc_configs(
    state: tauri::State<'_, AuthState>,
) -> Result<Vec<OidcConfig>, String> {
    let configs = state
        .configs
        .try_read()
        .map_err(|_| "Auth state lock poisoned".to_string())?;
    Ok(configs.clone())
}

/// Remove the OIDC configuration identified by `provider_name`.
#[tauri::command]
pub fn auth_delete_oidc_config(
    provider_name: String,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let mut configs = state
        .configs
        .try_write()
        .map_err(|_| "Auth state lock poisoned".to_string())?;
    let len_before = configs.len();
    configs.retain(|c| c.provider_name != provider_name);
    if configs.len() == len_before {
        Err(format!("No OIDC config found for provider '{provider_name}'"))
    } else {
        Ok(())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifier must be exactly 43 characters (32 raw bytes → base64url without
    /// padding) and consist solely of unreserved URL characters (A-Z, a-z, 0-9,
    /// `-`, `_`).  Challenge must be 43 characters with the same charset.
    #[test]
    fn test_generate_pkce_pair_length_and_charset() {
        let (verifier, challenge) = generate_pkce_pair();
        assert_eq!(
            verifier.len(),
            43,
            "verifier should be 43 base64url chars"
        );
        assert_eq!(
            challenge.len(),
            43,
            "challenge should be 43 base64url chars"
        );
        let valid = |s: &str| s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_');
        assert!(valid(&verifier), "verifier contains non-base64url chars: {verifier}");
        assert!(valid(&challenge), "challenge contains non-base64url chars: {challenge}");
    }

    /// The authorization URL must contain all six required OIDC/PKCE query parameters.
    #[test]
    fn test_build_auth_url_contains_required_params() {
        let config = OidcConfig {
            provider_name: "Test IdP".to_owned(),
            client_id: "my-client-id".to_owned(),
            authorization_endpoint: "https://idp.example.com/auth".to_owned(),
            token_endpoint: "https://idp.example.com/token".to_owned(),
            userinfo_endpoint: None,
            scopes: vec!["openid".to_owned(), "email".to_owned()],
        };
        let url = build_auth_url(
            &config,
            "http://127.0.0.1:12300/callback",
            "challenge_value",
            "random_state",
        );

        assert!(url.starts_with("https://idp.example.com/auth?"), "URL must start with the authorization endpoint");
        assert!(url.contains("response_type=code"), "missing response_type");
        assert!(url.contains("client_id=my-client-id"), "missing client_id");
        assert!(url.contains("redirect_uri="), "missing redirect_uri");
        assert!(url.contains("code_challenge=challenge_value"), "missing code_challenge");
        assert!(url.contains("code_challenge_method=S256"), "missing code_challenge_method");
        assert!(url.contains("state=random_state"), "missing state");
    }

    /// The challenge must equal BASE64URL(SHA-256(verifier_bytes)).
    #[test]
    fn test_pkce_challenge_is_sha256_of_verifier() {
        let (verifier, challenge) = generate_pkce_pair();

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let digest = hasher.finalize();
        let expected = URL_SAFE_NO_PAD.encode(digest);

        assert_eq!(
            challenge, expected,
            "PKCE challenge must be BASE64URL(SHA-256(verifier))"
        );
    }

    /// A well-formed JWT with a JSON payload must be decoded correctly.
    #[test]
    fn test_parse_id_token_claims_valid_jwt() {
        // Build a minimal JWT: header.payload.signature (signature is ignored).
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#);
        let payload_json = r#"{"sub":"user-123","email":"alice@example.com","name":"Alice"}"#;
        let payload = URL_SAFE_NO_PAD.encode(payload_json);
        let jwt = format!("{header}.{payload}.fakesignature");

        let claims = parse_id_token_claims(&jwt).expect("should parse valid JWT");
        assert_eq!(claims["sub"], "user-123");
        assert_eq!(claims["email"], "alice@example.com");
        assert_eq!(claims["name"], "Alice");
    }

    /// A string that is not a valid three-segment JWT must produce an error.
    #[test]
    fn test_parse_id_token_claims_invalid_jwt_returns_error() {
        let result = parse_id_token_claims("not.a.valid.jwt.at.all");
        // splitn(3, '.') with 4 actual dots yields only 3 parts but the middle
        // segment is still garbage base64 → JSON parse failure.
        assert!(
            result.is_err(),
            "expected error for malformed JWT, got Ok"
        );

        let result2 = parse_id_token_claims("onlyone");
        assert!(
            result2.is_err(),
            "expected error for single-segment token, got Ok"
        );
    }

    /// Verify that `url_encode` round-trips through `percent_decode`.
    #[test]
    fn test_url_encode_and_percent_decode_roundtrip() {
        let original = "http://127.0.0.1:12300/callback?foo=bar&baz=qux quux";
        let encoded = url_encode(original);
        // encoded must not contain spaces or bare slashes/colons/question marks.
        assert!(!encoded.contains(' '), "encoded should not have spaces");
        let decoded = percent_decode(&encoded);
        assert_eq!(decoded, original);
    }

    /// Verify `extract_query_param` works for a typical callback path.
    #[test]
    fn test_extract_query_param_callback() {
        let path = "/callback?code=AUTH_CODE_XYZ&state=RANDOM_STATE_ABC";
        assert_eq!(
            extract_query_param(path, "code").as_deref(),
            Some("AUTH_CODE_XYZ")
        );
        assert_eq!(
            extract_query_param(path, "state").as_deref(),
            Some("RANDOM_STATE_ABC")
        );
        assert!(extract_query_param(path, "missing").is_none());
    }
}
