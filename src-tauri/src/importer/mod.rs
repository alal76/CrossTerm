//! Import wizard backend: discovers and parses external SSH session sources.
//!
//! Exposes two Tauri commands:
//!   - `import_detect_sources`  – lists available import sources with session counts
//!   - `import_parse_source`    – parses a named source and returns structured sessions

pub mod bundle;

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Public Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ImportSource {
    pub source_type: ImportSourceType,
    pub display_name: String,
    pub path: Option<String>,
    pub session_count: usize,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceType {
    SshConfig,
    PuttyRegistry, // Windows only
    #[allow(dead_code)]
    SecureCrt,
    #[allow(dead_code)]
    MobaXterm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedSession {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub identity_file: Option<String>,
    pub jump_host: Option<String>,
    pub session_type: String, // always "ssh" for now
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct ImportResult {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub sessions: Vec<ImportedSession>,
}

// ── SSH Config Parser ─────────────────────────────────────────────────────────

/// Parses an OpenSSH `~/.ssh/config`-format file and returns one
/// [`ImportedSession`] per non-wildcard `Host` alias found.
///
/// Handles:
/// - `Host <alias>` blocks (and multi-alias `Host a b c`)
/// - `HostName`, `Port`, `User`, `IdentityFile`, `ProxyJump`, `ProxyCommand`
/// - `#` comments
/// - `Host *` wildcard blocks (skipped)
/// - Malformed `Port` values (defaults to 22)
pub fn parse_ssh_config(path: &Path) -> Vec<ImportedSession> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    parse_ssh_config_str(&content)
}

/// Inner implementation that works on an already-loaded string so tests can
/// call it without touching the filesystem.
fn parse_ssh_config_str(content: &str) -> Vec<ImportedSession> {
    // Each entry holds aliases + accumulated key-value pairs for one block.
    struct Block {
        aliases: Vec<String>,
        hostname: Option<String>,
        port: Option<u16>,
        user: Option<String>,
        identity_file: Option<String>,
        jump_host: Option<String>,
    }

    impl Block {
        fn new(aliases: Vec<String>) -> Self {
            Block {
                aliases,
                hostname: None,
                port: None,
                user: None,
                identity_file: None,
                jump_host: None,
            }
        }
    }

    let mut blocks: Vec<Block> = Vec::new();

    for raw_line in content.lines() {
        // Strip inline comments: everything from the first unquoted `#` onward.
        // Simple approach: split at `#` that is preceded by whitespace or is at
        // column 0 (good enough for the SSH config format).
        let line = {
            let trimmed = raw_line.trim();
            // Remove full-line comments first
            if trimmed.starts_with('#') {
                continue;
            }
            // Strip trailing inline comment (space/tab before `#`)
            let without_comment = match trimmed.find(" #").or_else(|| trimmed.find("\t#")) {
                Some(pos) => trimmed[..pos].trim(),
                None => trimmed,
            };
            if without_comment.is_empty() {
                continue;
            }
            without_comment.to_string()
        };

        // Split into keyword and value.  SSH config allows both
        //   Keyword Value
        // and
        //   Keyword=Value
        let (keyword, value) = if let Some(eq_pos) = line.find('=') {
            // Check whether the `=` comes before the first space (i.e. it's the
            // separator, not part of a path value).
            let space_pos = line.find(' ').unwrap_or(line.len());
            if eq_pos < space_pos {
                let k = line[..eq_pos].trim();
                let v = line[eq_pos + 1..].trim();
                (k.to_string(), v.to_string())
            } else {
                // Space-separated
                let mut parts = line.splitn(2, char::is_whitespace);
                let k = parts.next().unwrap_or("").trim().to_string();
                let v = parts.next().unwrap_or("").trim().to_string();
                (k, v)
            }
        } else {
            let mut parts = line.splitn(2, char::is_whitespace);
            let k = parts.next().unwrap_or("").trim().to_string();
            let v = parts.next().unwrap_or("").trim().to_string();
            (k, v)
        };

        if keyword.is_empty() {
            continue;
        }

        match keyword.to_lowercase().as_str() {
            "host" => {
                // May be a multi-alias line: `Host web1 web2`
                let aliases: Vec<String> = value
                    .split_whitespace()
                    .filter(|a| !a.is_empty())
                    .map(String::from)
                    .collect();
                if !aliases.is_empty() {
                    blocks.push(Block::new(aliases));
                }
            }

            "hostname" => {
                if let Some(block) = blocks.last_mut() {
                    block.hostname = Some(value);
                }
            }

            "port" => {
                if let Some(block) = blocks.last_mut() {
                    // Non-numeric port: silently fall back to 22 (recorded as None
                    // and resolved at session-creation time).
                    block.port = value.trim().parse::<u16>().ok();
                }
            }

            "user" => {
                if let Some(block) = blocks.last_mut() {
                    if block.user.is_none() {
                        block.user = Some(value);
                    }
                }
            }

            "identityfile" => {
                if let Some(block) = blocks.last_mut() {
                    if block.identity_file.is_none() {
                        block.identity_file = Some(value);
                    }
                }
            }

            "proxyjump" => {
                if let Some(block) = blocks.last_mut() {
                    if block.jump_host.is_none() {
                        // ProxyJump may be a comma-separated chain; take the first hop.
                        let first_hop = value
                            .split(',')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if !first_hop.is_empty() {
                            block.jump_host = Some(first_hop);
                        }
                    }
                }
            }

            "proxycommand" => {
                if let Some(block) = blocks.last_mut() {
                    if block.jump_host.is_none() {
                        // Heuristic: look for `-W %h:%p` or `ssh <host>` patterns.
                        // We try to extract the first argument that looks like a
                        // hostname (not a flag, not a %% token, not a path).
                        let extracted = extract_proxycommand_host(&value);
                        block.jump_host = extracted;
                    }
                }
            }

            // All other directives are intentionally ignored.
            _ => {}
        }
    }

    // Convert blocks into ImportedSession entries, skipping wildcard blocks.
    let mut sessions: Vec<ImportedSession> = Vec::new();

    for block in blocks {
        // Skip wildcard `Host *` and purely-glob patterns.
        let non_wildcard_aliases: Vec<&String> = block
            .aliases
            .iter()
            .filter(|a| !is_wildcard_only(a))
            .collect();

        if non_wildcard_aliases.is_empty() {
            continue;
        }

        for alias in non_wildcard_aliases {
            // The "host" to connect to: HostName overrides the alias.
            let host = block
                .hostname
                .clone()
                .unwrap_or_else(|| alias.clone());

            sessions.push(ImportedSession {
                name: alias.clone(),
                host,
                port: block.port.unwrap_or(22),
                username: block.user.clone(),
                identity_file: block.identity_file.clone(),
                jump_host: block.jump_host.clone(),
                session_type: "ssh".to_string(),
                tags: Vec::new(),
            });
        }
    }

    sessions
}

/// Returns `true` if the alias consists entirely of glob wildcards (`*` / `?`)
/// and therefore represents a catch-all defaults block that should be skipped.
fn is_wildcard_only(alias: &str) -> bool {
    !alias.is_empty() && (alias.contains('*') || alias.contains('?'))
}

/// Attempts to extract a jump-host name from a `ProxyCommand` value.
///
/// Handles common patterns such as:
///   - `ssh -W %h:%p bastion.example.com`
///   - `ssh bastion.example.com -W %h:%p`
///   - `nc bastion.example.com 22`
fn extract_proxycommand_host(proxycommand: &str) -> Option<String> {
    let tokens: Vec<&str> = proxycommand.split_whitespace().collect();
    // Skip the command name (index 0) and look for the first token that
    // looks like a hostname (not a flag, not a %-token, not a pure number).
    let mut skip_next = false;
    for token in tokens.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        // Single-letter flags take their argument as the next token.
        if token.starts_with('-') && token.len() == 2 {
            skip_next = true;
            continue;
        }
        // Multi-char flags like -o, -W, etc. are also one token; skip their arg.
        if token.starts_with('-') {
            skip_next = true;
            continue;
        }
        // Skip %-tokens (e.g. %h, %p, %r).
        if token.contains('%') {
            continue;
        }
        // Skip pure integers (port numbers).
        if token.parse::<u32>().is_ok() {
            continue;
        }
        // Skip tokens that are file paths.
        if token.starts_with('/') || token.starts_with('~') {
            continue;
        }
        // Whatever is left is likely a hostname.
        return Some(token.to_string());
    }
    None
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

/// Lists all known import sources, reporting which are available on the current
/// platform and how many sessions each contains.
#[tauri::command]
pub fn import_detect_sources() -> Vec<ImportSource> {
    let mut sources = Vec::new();

    // ~/.ssh/config
    if let Some(home) = dirs::home_dir() {
        let ssh_config = home.join(".ssh").join("config");
        let sessions = if ssh_config.exists() {
            parse_ssh_config(&ssh_config)
        } else {
            Vec::new()
        };
        sources.push(ImportSource {
            source_type: ImportSourceType::SshConfig,
            display_name: "OpenSSH Config (~/.ssh/config)".into(),
            path: ssh_config.to_str().map(String::from),
            session_count: sessions.len(),
            available: ssh_config.exists(),
        });
    }

    // PuTTY (Windows only) – always report as unavailable on non-Windows.
    sources.push(ImportSource {
        source_type: ImportSourceType::PuttyRegistry,
        display_name: "PuTTY Sessions (Windows Registry)".into(),
        path: None,
        session_count: 0,
        available: cfg!(target_os = "windows"),
    });

    sources
}

/// Parses the requested import source and returns a list of discovered sessions.
///
/// Currently supported `source_type` values:
///   - `"ssh_config"` – parses `~/.ssh/config`
#[tauri::command]
pub fn import_parse_source(source_type: String) -> Result<Vec<ImportedSession>, String> {
    match source_type.as_str() {
        "ssh_config" => {
            if let Some(home) = dirs::home_dir() {
                let path = home.join(".ssh").join("config");
                if path.exists() {
                    Ok(parse_ssh_config(&path))
                } else {
                    Err("~/.ssh/config not found".into())
                }
            } else {
                Err("Cannot determine home directory".into())
            }
        }
        _ => Err(format!("Unknown source type: {source_type}")),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn write_temp(content: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().expect("tempfile");
        f.write_all(content.as_bytes()).expect("write");
        f
    }

    fn parse_file(content: &str) -> Vec<ImportedSession> {
        let f = write_temp(content);
        parse_ssh_config(f.path())
    }

    // ── Test 1: canonical fixture ─────────────────────────────────────────

    #[test]
    fn test_canonical_fixture() {
        let fixture = r#"
Host webserver
    HostName 192.168.1.10
    Port 2222
    User deploy
    IdentityFile ~/.ssh/id_ed25519

Host bastion
    HostName bastion.example.com
    User ec2-user

Host internal
    HostName 10.0.0.5
    ProxyJump bastion

Host *
    ServerAliveInterval 60
"#;
        let sessions = parse_file(fixture);

        // Exactly 3 sessions (Host * skipped)
        assert_eq!(sessions.len(), 3, "Expected 3 sessions, got: {:?}", sessions.iter().map(|s| &s.name).collect::<Vec<_>>());

        let webserver = sessions.iter().find(|s| s.name == "webserver").expect("webserver missing");
        assert_eq!(webserver.host, "192.168.1.10");
        assert_eq!(webserver.port, 2222);
        assert_eq!(webserver.username.as_deref(), Some("deploy"));
        assert_eq!(webserver.identity_file.as_deref(), Some("~/.ssh/id_ed25519"));
        assert_eq!(webserver.session_type, "ssh");

        let bastion = sessions.iter().find(|s| s.name == "bastion").expect("bastion missing");
        assert_eq!(bastion.port, 22, "bastion should default to port 22");
        assert_eq!(bastion.host, "bastion.example.com");

        let internal = sessions.iter().find(|s| s.name == "internal").expect("internal missing");
        assert_eq!(internal.jump_host.as_deref(), Some("bastion"));
    }

    // ── Test 2: multi-alias Host line ─────────────────────────────────────

    #[test]
    fn test_multi_alias_host_line() {
        let config = r#"
Host web1 web2
    HostName 10.0.0.1
    User admin
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 2, "multi-alias should produce 2 sessions");
        let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"web1"), "web1 missing");
        assert!(names.contains(&"web2"), "web2 missing");
        // Both entries share the same resolved host
        for s in &sessions {
            assert_eq!(s.host, "10.0.0.1");
            assert_eq!(s.username.as_deref(), Some("admin"));
        }
    }

    // ── Test 3: comment-only file ─────────────────────────────────────────

    #[test]
    fn test_comment_only_file() {
        let config = r#"
# This is a comment
# Another comment

# End of file
"#;
        let sessions = parse_file(config);
        assert!(sessions.is_empty(), "comment-only file must return empty vec");
    }

    // ── Test 4: malformed port defaults to 22 ────────────────────────────

    #[test]
    fn test_malformed_port_defaults_to_22() {
        let config = r#"
Host badport
    HostName example.com
    Port notanumber
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].port, 22, "malformed port must default to 22");
    }

    // ── Test 5: ProxyCommand hostname extraction ──────────────────────────

    #[test]
    fn test_proxycommand_host_extraction() {
        let config = r#"
Host tunnel
    HostName 10.0.0.5
    ProxyCommand ssh -W %h:%p jump.example.com
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].jump_host.as_deref(), Some("jump.example.com"));
    }

    // ── Test 6: HostName falls back to alias when absent ─────────────────

    #[test]
    fn test_hostname_falls_back_to_alias() {
        let config = r#"
Host myserver
    User root
    Port 2200
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        // No HostName directive → host equals the alias
        assert_eq!(sessions[0].host, "myserver");
        assert_eq!(sessions[0].port, 2200);
    }

    // ── Test 7: empty file ────────────────────────────────────────────────

    #[test]
    fn test_empty_file() {
        let sessions = parse_file("");
        assert!(sessions.is_empty(), "empty file must return empty vec");
    }

    // ── Test 8: Host ? (single-char wildcard) is skipped ─────────────────

    #[test]
    fn test_single_char_wildcard_skipped() {
        let config = r#"
Host ?
    Port 22

Host realhost
    HostName 1.2.3.4
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1, "Host ? should be skipped");
        assert_eq!(sessions[0].name, "realhost");
    }

    // ── Test 9: equals-sign keyword separator ────────────────────────────

    #[test]
    fn test_equals_sign_separator() {
        let config = r#"
Host=eqserver
    HostName=eq.example.com
    Port=2222
    User=sysop
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.name, "eqserver");
        assert_eq!(s.host, "eq.example.com");
        assert_eq!(s.port, 2222);
        assert_eq!(s.username.as_deref(), Some("sysop"));
    }

    // ── Test 10: ProxyJump chain – only first hop retained ────────────────

    #[test]
    fn test_proxyjump_chain_first_hop() {
        let config = r#"
Host deep
    HostName 192.168.100.5
    ProxyJump hop1.example.com,hop2.example.com
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        assert_eq!(
            sessions[0].jump_host.as_deref(),
            Some("hop1.example.com"),
            "should keep only the first ProxyJump hop"
        );
    }

    // ── Test 11: inline comment stripped ────────────────────────────────

    #[test]
    fn test_inline_comment_stripped() {
        let config = r#"
Host commented
    HostName 10.1.2.3 # production server
    Port 22 # default
    User ops # service account
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        // HostName must not contain the comment text
        assert_eq!(s.host, "10.1.2.3");
        assert_eq!(s.username.as_deref(), Some("ops"));
    }

    // ── Test 12: multiple blocks, only non-wildcard counted ───────────────

    #[test]
    fn test_multiple_wildcards_skipped() {
        let config = r#"
Host dev
    HostName dev.internal

Host *
    ServerAliveInterval 30

Host *.example.com
    User shared

Host prod
    HostName prod.internal
"#;
        let sessions = parse_file(config);
        // "Host *" and "Host *.example.com" must both be skipped
        let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(sessions.len(), 2, "Expected only dev and prod, got: {:?}", names);
        assert!(names.contains(&"dev"));
        assert!(names.contains(&"prod"));
    }

    // ── Test 13: session_type is always "ssh" ────────────────────────────

    #[test]
    fn test_session_type_always_ssh() {
        let config = r#"
Host alpha
    HostName alpha.example.com

Host beta
    HostName beta.example.com
    Port 2222
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 2);
        for s in &sessions {
            assert_eq!(s.session_type, "ssh", "session_type must always be 'ssh'");
        }
    }

    // ── Test 14: tags vec is empty by default ────────────────────────────

    #[test]
    fn test_tags_empty_by_default() {
        let config = r#"
Host tagtest
    HostName 1.2.3.4
"#;
        let sessions = parse_file(config);
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].tags.is_empty(), "tags should be empty by default");
    }
}
