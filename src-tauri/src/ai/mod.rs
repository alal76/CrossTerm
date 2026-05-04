use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock};
use std::time::Duration;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Safe,
    Caution,
    Dangerous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSuggestion {
    pub command: String,
    pub explanation: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContext {
    pub current_directory: Option<String>,
    pub shell: Option<String>,
    pub os: Option<String>,
    pub recent_commands: Vec<String>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct AiState {
    pub ollama_url: Arc<RwLock<String>>,
    pub model: Arc<RwLock<String>>,
    pub available: Arc<RwLock<Option<bool>>>,
}

impl AiState {
    pub fn new() -> Self {
        Self {
            ollama_url: Arc::new(RwLock::new("http://localhost:11434".to_string())),
            model: Arc::new(RwLock::new("llama3.2".to_string())),
            available: Arc::new(RwLock::new(None)),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Parse a URL string into (host, port). Returns None if the URL cannot be parsed.
fn parse_url_host_port(url: &str) -> Option<(String, u16)> {
    // Strip scheme prefix (http:// or https://)
    let without_scheme = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        url
    };

    // Strip any path component
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);

    // Split host and port
    if let Some(colon_pos) = host_port.rfind(':') {
        let host = &host_port[..colon_pos];
        let port_str = &host_port[colon_pos + 1..];
        if let Ok(port) = port_str.parse::<u16>() {
            return Some((host.to_string(), port));
        }
    }

    // Default port based on scheme
    let default_port = if url.starts_with("https://") { 443 } else { 80 };
    Some((host_port.to_string(), default_port))
}

// ── Core Functions ───────────────────────────────────────────────────────

/// Check if Ollama is running by attempting a TCP connect to the configured URL.
pub fn check_ollama_available(url: &str) -> bool {
    let Some((host, port)) = parse_url_host_port(url) else {
        return false;
    };
    let addr = format!("{host}:{port}");
    TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap()),
        Duration::from_secs(2),
    )
    .is_ok()
}

/// Send a prompt to Ollama using the `/api/generate` endpoint.
/// Returns the full response text by concatenating all "response" fields
/// from the NDJSON stream.
pub fn ollama_generate(url: &str, model: &str, prompt: &str) -> Result<String, String> {
    let (host, port) = parse_url_host_port(url)
        .ok_or_else(|| format!("Invalid Ollama URL: {url}"))?;

    let addr = format!("{host}:{port}");
    let sock_addr: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("Invalid address {addr}: {e}"))?;

    let mut stream = TcpStream::connect_timeout(&sock_addr, Duration::from_secs(10))
        .map_err(|e| format!("Cannot connect to Ollama at {addr}: {e}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;

    // Build JSON body
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": true
    })
    .to_string();

    let request = format!(
        "POST /api/generate HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        host = host,
        port = port,
        len = body.len(),
        body = body
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Failed to send request: {e}"))?;

    // Read full response
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("Failed to read response: {e}"))?;

    let response_str =
        String::from_utf8_lossy(&raw).to_string();

    // Split HTTP headers from body
    let body_str = if let Some(pos) = response_str.find("\r\n\r\n") {
        &response_str[pos + 4..]
    } else if let Some(pos) = response_str.find("\n\n") {
        &response_str[pos + 2..]
    } else {
        &response_str
    };

    // Handle chunked transfer encoding: strip chunk size lines
    // Each chunk is: <hex-size>\r\n<data>\r\n
    let ndjson_body = if response_str.contains("Transfer-Encoding: chunked")
        || response_str.contains("transfer-encoding: chunked")
    {
        decode_chunked(body_str)
    } else {
        body_str.to_string()
    };

    // Parse NDJSON: each line is a JSON object with a "response" field
    let mut full_response = String::new();
    for line in ndjson_body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(fragment) = val.get("response").and_then(|r| r.as_str()) {
                full_response.push_str(fragment);
            }
            // If this is the done marker with an error field, surface it
            if val.get("error").is_some() {
                return Err(format!(
                    "Ollama error: {}",
                    val["error"].as_str().unwrap_or("unknown")
                ));
            }
        }
    }

    if full_response.is_empty() {
        return Err(format!(
            "Empty response from Ollama. Raw body: {}",
            &ndjson_body[..ndjson_body.len().min(200)]
        ));
    }

    Ok(full_response)
}

/// Decode HTTP chunked transfer encoding.
fn decode_chunked(input: &str) -> String {
    let mut result = String::new();
    let mut lines = input.lines().peekable();
    while let Some(size_line) = lines.next() {
        let size_str = size_line.trim().split(';').next().unwrap_or("").trim();
        if let Ok(chunk_size) = u64::from_str_radix(size_str, 16) {
            if chunk_size == 0 {
                break;
            }
            // Collect bytes for this chunk
            let mut collected = 0u64;
            while collected < chunk_size {
                if let Some(data_line) = lines.next() {
                    result.push_str(data_line);
                    result.push('\n');
                    collected += data_line.len() as u64 + 1;
                } else {
                    break;
                }
            }
        }
    }
    if result.is_empty() {
        input.to_string()
    } else {
        result
    }
}

/// Build the system prompt for command suggestions.
fn build_command_prompt(user_request: &str, context: &AiContext) -> String {
    let os = context.os.as_deref().unwrap_or("linux");
    let shell = context.shell.as_deref().unwrap_or("bash");
    let cwd = context
        .current_directory
        .as_deref()
        .unwrap_or("unknown");

    let recent = if context.recent_commands.is_empty() {
        "none".to_string()
    } else {
        context.recent_commands.join(", ")
    };

    format!(
        r#"You are a terminal command assistant. The user is on {os} using {shell} shell in directory: {cwd}.
Recent commands: {recent}

Suggest 1-3 terminal commands for the following request: "{user_request}"

Respond ONLY with a JSON array of objects, no other text. Each object must have exactly these fields:
- "command": the shell command string
- "explanation": a short plain English explanation (1-2 sentences)
- "risk_level": one of "safe", "caution", or "dangerous"

Example:
[{{"command": "ls -la", "explanation": "List all files including hidden ones with details.", "risk_level": "safe"}}]"#
    )
}

/// Parse Ollama's text response into CommandSuggestion structs.
/// Tries JSON array extraction first, then falls back to structured text parsing.
fn parse_suggestions(text: &str) -> Vec<CommandSuggestion> {
    // Try to extract a JSON array from the response
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            let json_slice = &text[start..=end];
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json_slice) {
                let suggestions: Vec<CommandSuggestion> = arr
                    .into_iter()
                    .filter_map(|v| {
                        let command = v.get("command")?.as_str()?.to_string();
                        let explanation = v
                            .get("explanation")
                            .and_then(|e| e.as_str())
                            .unwrap_or("No explanation provided.")
                            .to_string();
                        let risk_level = match v
                            .get("risk_level")
                            .and_then(|r| r.as_str())
                            .unwrap_or("safe")
                        {
                            "dangerous" => RiskLevel::Dangerous,
                            "caution" => RiskLevel::Caution,
                            _ => RiskLevel::Safe,
                        };
                        Some(CommandSuggestion {
                            command,
                            explanation,
                            risk_level,
                        })
                    })
                    .collect();
                if !suggestions.is_empty() {
                    return suggestions;
                }
            }
        }
    }

    // Fallback: treat the whole text as a single command suggestion
    if !text.trim().is_empty() {
        return vec![CommandSuggestion {
            command: text.lines().next().unwrap_or(text.trim()).to_string(),
            explanation: "AI-generated command suggestion.".to_string(),
            risk_level: RiskLevel::Caution,
        }];
    }

    vec![]
}

// ── Tauri Commands ───────────────────────────────────────────────────────

#[tauri::command]
pub fn ai_check_available(state: tauri::State<'_, AiState>) -> bool {
    let url = state
        .ollama_url
        .read()
        .map(|u| u.clone())
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let result = check_ollama_available(&url);

    if let Ok(mut avail) = state.available.write() {
        *avail = Some(result);
    }

    result
}

#[tauri::command]
pub fn ai_suggest_command(
    user_request: String,
    context: AiContext,
    state: tauri::State<'_, AiState>,
) -> Result<Vec<CommandSuggestion>, String> {
    let url = state
        .ollama_url
        .read()
        .map(|u| u.clone())
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = state
        .model
        .read()
        .map(|m| m.clone())
        .unwrap_or_else(|_| "llama3.2".to_string());

    let prompt = build_command_prompt(&user_request, &context);
    let response = ollama_generate(&url, &model, &prompt)?;
    let suggestions = parse_suggestions(&response);

    if suggestions.is_empty() {
        return Err("No suggestions could be parsed from the AI response.".to_string());
    }

    Ok(suggestions)
}

#[tauri::command]
pub fn ai_explain_output(
    command: String,
    output: String,
    state: tauri::State<'_, AiState>,
) -> Result<String, String> {
    let url = state
        .ollama_url
        .read()
        .map(|u| u.clone())
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = state
        .model
        .read()
        .map(|m| m.clone())
        .unwrap_or_else(|_| "llama3.2".to_string());

    let prompt = format!(
        "Explain the following terminal command output in plain English. Be concise (2-4 sentences).\n\nCommand: {command}\n\nOutput:\n{output}\n\nExplanation:"
    );

    ollama_generate(&url, &model, &prompt)
}

#[tauri::command]
pub fn ai_set_model(model: String, state: tauri::State<'_, AiState>) -> Result<(), String> {
    if model.trim().is_empty() {
        return Err("Model name cannot be empty.".to_string());
    }
    if let Ok(mut m) = state.model.write() {
        *m = model;
        Ok(())
    } else {
        Err("Failed to acquire model lock.".to_string())
    }
}

#[tauri::command]
pub fn ai_get_config(state: tauri::State<'_, AiState>) -> serde_json::Value {
    let url = state
        .ollama_url
        .read()
        .map(|u| u.clone())
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let model = state
        .model
        .read()
        .map(|m| m.clone())
        .unwrap_or_else(|_| "llama3.2".to_string());
    let available = state
        .available
        .read()
        .map(|a| *a)
        .unwrap_or(None);

    serde_json::json!({
        "ollama_url": url,
        "model": model,
        "available": available,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_command_prompt_includes_context() {
        let context = AiContext {
            current_directory: Some("/home/user/projects".to_string()),
            shell: Some("zsh".to_string()),
            os: Some("linux".to_string()),
            recent_commands: vec!["ls".to_string(), "git status".to_string()],
        };
        let prompt = build_command_prompt("list all running processes", &context);

        assert!(prompt.contains("linux"), "prompt should mention OS");
        assert!(prompt.contains("zsh"), "prompt should mention shell");
        assert!(
            prompt.contains("/home/user/projects"),
            "prompt should include working directory"
        );
        assert!(
            prompt.contains("ls"),
            "prompt should include recent commands"
        );
        assert!(
            prompt.contains("list all running processes"),
            "prompt should include the user request"
        );
        assert!(
            prompt.contains("JSON array"),
            "prompt should request JSON array output"
        );
    }

    #[test]
    fn test_check_ollama_available_false_when_not_running() {
        // Port 1 is always closed / unreachable in normal circumstances
        let result = check_ollama_available("http://127.0.0.1:1");
        assert!(
            !result,
            "should return false when nothing is listening on port 1"
        );
    }

    #[test]
    fn test_risk_level_serializes_snake_case() {
        let safe = serde_json::to_string(&RiskLevel::Safe).unwrap();
        let caution = serde_json::to_string(&RiskLevel::Caution).unwrap();
        let dangerous = serde_json::to_string(&RiskLevel::Dangerous).unwrap();

        assert_eq!(safe, r#""safe""#);
        assert_eq!(caution, r#""caution""#);
        assert_eq!(dangerous, r#""dangerous""#);
    }

    #[test]
    fn test_ai_state_default_model_is_llama() {
        let state = AiState::new();
        let model = state.model.read().unwrap();
        assert_eq!(*model, "llama3.2");

        let url = state.ollama_url.read().unwrap();
        assert!(url.contains("localhost:11434"));

        let available = state.available.read().unwrap();
        assert!(*available == None, "available should be None on init");
    }

    #[test]
    fn test_parse_suggestions_valid_json_array() {
        let text = r#"Here are some suggestions:
[{"command": "ps aux", "explanation": "List all running processes.", "risk_level": "safe"},
 {"command": "kill -9 1234", "explanation": "Force kill a process by PID.", "risk_level": "dangerous"}]
"#;
        let suggestions = parse_suggestions(text);
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].command, "ps aux");
        assert_eq!(suggestions[0].risk_level, RiskLevel::Safe);
        assert_eq!(suggestions[1].command, "kill -9 1234");
        assert_eq!(suggestions[1].risk_level, RiskLevel::Dangerous);
    }

    #[test]
    fn test_parse_suggestions_fallback_plain_text() {
        let text = "ls -la\nsome other text";
        let suggestions = parse_suggestions(text);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].command, "ls -la");
        assert_eq!(suggestions[0].risk_level, RiskLevel::Caution);
    }

    #[test]
    fn test_parse_url_host_port() {
        assert_eq!(
            parse_url_host_port("http://localhost:11434"),
            Some(("localhost".to_string(), 11434))
        );
        assert_eq!(
            parse_url_host_port("http://127.0.0.1:8080/api"),
            Some(("127.0.0.1".to_string(), 8080))
        );
        assert_eq!(
            parse_url_host_port("http://example.com"),
            Some(("example.com".to_string(), 80))
        );
    }
}
