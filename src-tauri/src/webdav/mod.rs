/// WebDAV file browser using reqwest for PROPFIND/GET/PUT/DELETE/MKCOL.
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WebDavError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("HTTP {0}: {1}")]
    Http(u16, String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Request error: {0}")]
    Request(String),
    #[error("XML parse error: {0}")]
    #[allow(dead_code)] // reserved for future PROPFIND response validation
    Xml(String),
}

impl Serialize for WebDavError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub verify_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavSession {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebDavEntryType {
    File,
    Collection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavEntry {
    pub href: String,
    pub name: String,
    pub entry_type: WebDavEntryType,
    pub content_length: Option<u64>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
}

pub struct WebDavState {
    sessions: Mutex<HashMap<String, (WebDavConfig, reqwest::Client)>>,
}

impl WebDavState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

fn build_client(verify_tls: bool) -> Result<reqwest::Client, WebDavError> {
    let mut b = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30));
    if !verify_tls { b = b.danger_accept_invalid_certs(true); }
    b.build().map_err(|e| WebDavError::Request(e.to_string()))
}

/// Parses a multistatus PROPFIND response. Note: this matches tags by the
/// literal `D:` prefix (as sent in this module's own PROPFIND_BODY request
/// and commonly echoed back by IIS/Apache mod_dav), not by resolving the
/// `DAV:` namespace URI — per the XML spec a server is free to use any
/// prefix (or none), so a server that replies with e.g. `d:` or `lp1:`-
/// prefixed elements (as some WebDAV implementations do) would parse to an
/// empty list here. Documented rather than silently assumed to be complete.
fn parse_propfind(xml: &str, _base_url: &str) -> Vec<WebDavEntry> {
    let mut entries = Vec::new();
    for response in xml.split("<D:response>").skip(1) {
        let href = response.split("<D:href>").nth(1)
            .and_then(|s| s.split("</D:href>").next())
            .unwrap_or("").trim().to_string();
        let name = href.trim_end_matches('/').split('/').next_back()
            .unwrap_or(&href).to_string();
        let is_collection = response.contains("<D:collection");
        let content_length = response.split("<D:getcontentlength>").nth(1)
            .and_then(|s| s.split("</D:getcontentlength>").next())
            .and_then(|s| s.trim().parse().ok());
        let last_modified = response.split("<D:getlastmodified>").nth(1)
            .and_then(|s| s.split("</D:getlastmodified>").next())
            .map(str::to_string);
        let content_type = response.split("<D:getcontenttype>").nth(1)
            .and_then(|s| s.split("</D:getcontenttype>").next())
            .map(str::to_string);

        if !href.is_empty() {
            entries.push(WebDavEntry {
                href,
                name,
                entry_type: if is_collection { WebDavEntryType::Collection } else { WebDavEntryType::File },
                content_length,
                last_modified,
                content_type,
            });
        }
    }
    entries
}

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:resourcetype/><D:getcontentlength/><D:getlastmodified/><D:getcontenttype/>
  </D:prop>
</D:propfind>"#;

#[tauri::command]
pub async fn webdav_connect(
    config: WebDavConfig,
    state: tauri::State<'_, WebDavState>,
) -> Result<String, WebDavError> {
    let client = build_client(config.verify_tls)?;
    let mut req = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), &config.url)
        .header("Depth", "0")
        .header("Content-Type", "application/xml")
        .body(PROPFIND_BODY);

    if let (Some(u), Some(p)) = (&config.username, &config.password) {
        req = req.basic_auth(u, Some(p));
    }

    let resp = req.send().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    let status = resp.status().as_u16();
    if status == 401 { return Err(WebDavError::AuthFailed); }
    if status >= 400 {
        return Err(WebDavError::Http(status, resp.text().await.unwrap_or_default()));
    }

    let id = Uuid::new_v4().to_string();
    state.sessions.lock().unwrap().insert(id.clone(), (config, client));
    Ok(id)
}

#[tauri::command]
pub async fn webdav_list(
    id: String,
    path: String,
    state: tauri::State<'_, WebDavState>,
) -> Result<Vec<WebDavEntry>, WebDavError> {
    let (cfg, client) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| WebDavError::NotFound(id.clone()))?;

    let url = format!("{}/{}", cfg.url.trim_end_matches('/'), path.trim_start_matches('/'));
    let mut req = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .header("Depth", "1")
        .header("Content-Type", "application/xml")
        .body(PROPFIND_BODY);

    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        req = req.basic_auth(u, Some(p));
    }

    let resp = req.send().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(WebDavError::Http(status, resp.text().await.unwrap_or_default()));
    }
    let xml = resp.text().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    Ok(parse_propfind(&xml, &cfg.url))
}

#[tauri::command]
pub async fn webdav_get(
    id: String,
    path: String,
    state: tauri::State<'_, WebDavState>,
) -> Result<Vec<u8>, WebDavError> {
    let (cfg, client) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| WebDavError::NotFound(id.clone()))?;

    let url = format!("{}/{}", cfg.url.trim_end_matches('/'), path.trim_start_matches('/'));
    let mut req = client.get(&url);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        req = req.basic_auth(u, Some(p));
    }
    let bytes = req.send().await.map_err(|e| WebDavError::Request(e.to_string()))?
        .bytes().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    Ok(bytes.to_vec())
}

#[tauri::command]
pub async fn webdav_put(
    id: String,
    path: String,
    content: Vec<u8>,
    content_type: Option<String>,
    state: tauri::State<'_, WebDavState>,
) -> Result<(), WebDavError> {
    let (cfg, client) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| WebDavError::NotFound(id.clone()))?;

    let url = format!("{}/{}", cfg.url.trim_end_matches('/'), path.trim_start_matches('/'));
    let ct = content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let mut req = client.put(&url).header("Content-Type", ct).body(content);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        req = req.basic_auth(u, Some(p));
    }
    let resp = req.send().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(WebDavError::Http(status, resp.text().await.unwrap_or_default()));
    }
    Ok(())
}

#[tauri::command]
pub async fn webdav_delete(
    id: String,
    path: String,
    state: tauri::State<'_, WebDavState>,
) -> Result<(), WebDavError> {
    let (cfg, client) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| WebDavError::NotFound(id.clone()))?;

    let url = format!("{}/{}", cfg.url.trim_end_matches('/'), path.trim_start_matches('/'));
    let mut req = client.delete(&url);
    if let (Some(u), Some(p)) = (&cfg.username, &cfg.password) {
        req = req.basic_auth(u, Some(p));
    }
    let resp = req.send().await.map_err(|e| WebDavError::Request(e.to_string()))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(WebDavError::Http(status, resp.text().await.unwrap_or_default()));
    }
    Ok(())
}

#[tauri::command]
pub fn webdav_disconnect(id: String, state: tauri::State<'_, WebDavState>) -> Result<(), WebDavError> {
    state.sessions.lock().unwrap().remove(&id).ok_or(WebDavError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn webdav_list_sessions(state: tauri::State<'_, WebDavState>) -> Vec<WebDavSession> {
    state.sessions.lock().unwrap().iter().map(|(id, (cfg, _))| WebDavSession {
        id: id.clone(), url: cfg.url.clone(),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_propfind_extracts_collection_and_file_entries() {
        let xml = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/docs/</D:href>
    <D:propstat><D:prop>
      <D:resourcetype><D:collection/></D:resourcetype>
      <D:getlastmodified>Mon, 01 Jan 2026 00:00:00 GMT</D:getlastmodified>
    </D:prop></D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/notes.txt</D:href>
    <D:propstat><D:prop>
      <D:resourcetype/>
      <D:getcontentlength>42</D:getcontentlength>
      <D:getcontenttype>text/plain</D:getcontenttype>
    </D:prop></D:propstat>
  </D:response>
</D:multistatus>"#;

        let entries = parse_propfind(xml, "http://example.com/dav");
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].href, "/dav/docs/");
        assert_eq!(entries[0].name, "docs");
        assert_eq!(entries[0].entry_type, WebDavEntryType::Collection);
        assert_eq!(entries[0].last_modified.as_deref(), Some("Mon, 01 Jan 2026 00:00:00 GMT"));
        assert_eq!(entries[0].content_length, None);

        assert_eq!(entries[1].href, "/dav/notes.txt");
        assert_eq!(entries[1].name, "notes.txt");
        assert_eq!(entries[1].entry_type, WebDavEntryType::File);
        assert_eq!(entries[1].content_length, Some(42));
        assert_eq!(entries[1].content_type.as_deref(), Some("text/plain"));
    }

    #[test]
    fn parse_propfind_returns_empty_for_a_response_with_no_entries() {
        let xml = r#"<D:multistatus xmlns:D="DAV:"></D:multistatus>"#;
        assert!(parse_propfind(xml, "http://example.com").is_empty());
    }

    #[test]
    fn parse_propfind_skips_a_malformed_response_missing_href() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:propstat><D:prop><D:getcontentlength>10</D:getcontentlength></D:prop></D:propstat>
  </D:response>
</D:multistatus>"#;
        assert!(parse_propfind(xml, "http://example.com").is_empty());
    }

    #[test]
    fn parse_propfind_strips_trailing_slash_when_deriving_the_display_name() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
  <D:response><D:href>/a/b/c/</D:href></D:response>
</D:multistatus>"#;
        let entries = parse_propfind(xml, "http://example.com");
        assert_eq!(entries[0].name, "c");
    }

    #[test]
    fn build_client_succeeds_with_and_without_tls_verification() {
        assert!(build_client(true).is_ok());
        assert!(build_client(false).is_ok());
    }

    #[test]
    fn parse_propfind_defaults_content_length_to_none_on_unparseable_value() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/f.bin</D:href>
    <D:propstat><D:prop><D:getcontentlength>not-a-number</D:getcontentlength></D:prop></D:propstat>
  </D:response>
</D:multistatus>"#;
        let entries = parse_propfind(xml, "http://example.com");
        assert_eq!(entries[0].content_length, None);
    }

    #[test]
    fn parse_propfind_handles_multiple_responses_in_one_document() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
  <D:response><D:href>/a/</D:href></D:response>
  <D:response><D:href>/b/</D:href></D:response>
  <D:response><D:href>/c/</D:href></D:response>
</D:multistatus>"#;
        let entries = parse_propfind(xml, "http://example.com");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }

    #[test]
    fn webdav_error_display_and_serialize() {
        let err = WebDavError::NotFound("sess1".into());
        assert_eq!(err.to_string(), "Session not found: sess1");
        assert_eq!(serde_json::to_string(&err).unwrap(), "\"Session not found: sess1\"");

        let err = WebDavError::Http(404, "Not Found".into());
        assert_eq!(err.to_string(), "HTTP 404: Not Found");

        let err = WebDavError::AuthFailed;
        assert_eq!(err.to_string(), "Authentication failed");
    }

    #[test]
    fn webdav_entry_type_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&WebDavEntryType::File).unwrap(), "\"file\"");
        assert_eq!(serde_json::to_string(&WebDavEntryType::Collection).unwrap(), "\"collection\"");
    }

    #[test]
    fn webdav_config_and_session_serde_round_trip() {
        let cfg = WebDavConfig {
            url: "https://dav.example.com".into(),
            username: Some("bob".into()),
            password: Some("hunter2".into()),
            verify_tls: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: WebDavConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.url, "https://dav.example.com");
        assert!(restored.verify_tls);

        let session = WebDavSession { id: "s1".into(), url: "https://dav.example.com".into() };
        let restored: WebDavSession = serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
        assert_eq!(restored.id, "s1");
    }

    #[test]
    fn webdav_state_new_starts_empty() {
        let state = WebDavState::new();
        assert!(state.sessions.lock().unwrap().is_empty());
    }
}
