use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Read error: {0}")]
    ReadError(String),
    #[error("Write error: {0}")]
    WriteError(String),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Diff error: {0}")]
    DiffError(String),
    #[error("Unsupported encoding: {0}")]
    UnsupportedEncoding(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Serialize for EditorError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorFile {
    pub id: String,
    pub path: String,
    pub content: String,
    pub encoding: String,
    pub language: Option<String>,
    pub modified: bool,
    pub line_count: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub left_path: String,
    pub right_path: String,
    pub hunks: Vec<DiffHunk>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub left_start: u32,
    pub left_count: u32,
    pub right_start: u32,
    pub right_count: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub content: String,
    pub left_line: Option<u32>,
    pub right_line: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub additions: u32,
    pub deletions: u32,
    pub modifications: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxToken {
    pub start: u32,
    pub end: u32,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub text: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn detect_language_from_extension(path: &str) -> String {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "jsx" => "javascriptreact",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "sh" | "bash" | "zsh" => "shell",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "cc" | "cxx" => "cpp",
        "h" | "hpp" => "cpp",
        "rb" => "ruby",
        "php" => "php",
        "sql" => "sql",
        "xml" => "xml",
        "txt" | "text" => "plaintext",
        "conf" | "cfg" | "ini" => "ini",
        "dockerfile" => "dockerfile",
        _ => "plaintext",
    }
    .into()
}

fn compute_diff(left_lines: &[&str], right_lines: &[&str]) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut left_idx: usize = 0;
    let mut right_idx: usize = 0;
    let mut current_lines: Vec<DiffLine> = Vec::new();
    let mut hunk_left_start: u32 = 1;
    let mut hunk_right_start: u32 = 1;
    let mut in_hunk = false;

    let max_len = std::cmp::max(left_lines.len(), right_lines.len());

    while left_idx < left_lines.len() || right_idx < right_lines.len() {
        let left_line = left_lines.get(left_idx).copied();
        let right_line = right_lines.get(right_idx).copied();

        match (left_line, right_line) {
            (Some(l), Some(r)) if l == r => {
                // Context line
                if in_hunk {
                    current_lines.push(DiffLine {
                        line_type: DiffLineType::Context,
                        content: l.to_string(),
                        left_line: Some(left_idx as u32 + 1),
                        right_line: Some(right_idx as u32 + 1),
                    });
                }
                left_idx += 1;
                right_idx += 1;
            }
            (Some(l), Some(r)) => {
                // Modified: line differs
                if !in_hunk {
                    in_hunk = true;
                    hunk_left_start = left_idx as u32 + 1;
                    hunk_right_start = right_idx as u32 + 1;
                    // Add up to 3 context lines before
                    let ctx_start = if left_idx >= 3 { left_idx - 3 } else { 0 };
                    for i in ctx_start..left_idx {
                        if let Some(cl) = left_lines.get(i) {
                            current_lines.push(DiffLine {
                                line_type: DiffLineType::Context,
                                content: cl.to_string(),
                                left_line: Some(i as u32 + 1),
                                right_line: Some(i as u32 + 1),
                            });
                        }
                    }
                    hunk_left_start = ctx_start as u32 + 1;
                    hunk_right_start = ctx_start as u32 + 1;
                }
                current_lines.push(DiffLine {
                    line_type: DiffLineType::Removed,
                    content: l.to_string(),
                    left_line: Some(left_idx as u32 + 1),
                    right_line: None,
                });
                current_lines.push(DiffLine {
                    line_type: DiffLineType::Added,
                    content: r.to_string(),
                    left_line: None,
                    right_line: Some(right_idx as u32 + 1),
                });
                left_idx += 1;
                right_idx += 1;
            }
            (Some(l), None) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_left_start = left_idx as u32 + 1;
                    hunk_right_start = right_idx as u32 + 1;
                }
                current_lines.push(DiffLine {
                    line_type: DiffLineType::Removed,
                    content: l.to_string(),
                    left_line: Some(left_idx as u32 + 1),
                    right_line: None,
                });
                left_idx += 1;
            }
            (None, Some(r)) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_left_start = left_idx as u32 + 1;
                    hunk_right_start = right_idx as u32 + 1;
                }
                current_lines.push(DiffLine {
                    line_type: DiffLineType::Added,
                    content: r.to_string(),
                    left_line: None,
                    right_line: Some(right_idx as u32 + 1),
                });
                right_idx += 1;
            }
            (None, None) => break,
        }

        // Flush hunk after a stretch of context following changes
        let next_differs = {
            let nl = left_lines.get(left_idx).copied();
            let nr = right_lines.get(right_idx).copied();
            match (nl, nr) {
                (Some(a), Some(b)) => a != b,
                (None, None) => false,
                _ => true,
            }
        };

        if in_hunk && !next_differs && (left_idx >= left_lines.len() && right_idx >= right_lines.len() || {
            // Check if we have enough trailing context
            let has_changes = current_lines.iter().any(|l| !matches!(l.line_type, DiffLineType::Context));
            has_changes && !next_differs
        }) {
            let left_count = current_lines
                .iter()
                .filter(|l| matches!(l.line_type, DiffLineType::Context | DiffLineType::Removed))
                .count() as u32;
            let right_count = current_lines
                .iter()
                .filter(|l| matches!(l.line_type, DiffLineType::Context | DiffLineType::Added))
                .count() as u32;

            if current_lines.iter().any(|l| !matches!(l.line_type, DiffLineType::Context)) {
                hunks.push(DiffHunk {
                    left_start: hunk_left_start,
                    left_count,
                    right_start: hunk_right_start,
                    right_count,
                    lines: std::mem::take(&mut current_lines),
                });
            } else {
                current_lines.clear();
            }
            in_hunk = false;
        }
    }

    // Flush remaining
    if !current_lines.is_empty()
        && current_lines
            .iter()
            .any(|l| !matches!(l.line_type, DiffLineType::Context))
    {
        let left_count = current_lines
            .iter()
            .filter(|l| matches!(l.line_type, DiffLineType::Context | DiffLineType::Removed))
            .count() as u32;
        let right_count = current_lines
            .iter()
            .filter(|l| matches!(l.line_type, DiffLineType::Context | DiffLineType::Added))
            .count() as u32;

        hunks.push(DiffHunk {
            left_start: hunk_left_start,
            left_count,
            right_start: hunk_right_start,
            right_count,
            lines: current_lines,
        });
    }

    hunks
}

fn compute_stats(hunks: &[DiffHunk]) -> DiffStats {
    let mut additions = 0u32;
    let mut deletions = 0u32;
    for hunk in hunks {
        for line in &hunk.lines {
            match line.line_type {
                DiffLineType::Added => additions += 1,
                DiffLineType::Removed => deletions += 1,
                DiffLineType::Context => {}
            }
        }
    }
    DiffStats {
        additions,
        deletions,
        modifications: 0,
    }
}

// ── State ───────────────────────────────────────────────────────────────

pub struct EditorState {
    open_files: Mutex<HashMap<String, EditorFile>>,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            open_files: Mutex::new(HashMap::new()),
        }
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn editor_open(
    path: String,
    state: tauri::State<'_, EditorState>,
) -> Result<EditorFile, EditorError> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(EditorError::FileNotFound(path));
    }

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| EditorError::ReadError(e.to_string()))?;
    let metadata = std::fs::metadata(file_path)
        .map_err(|e| EditorError::ReadError(e.to_string()))?;
    let language = detect_language_from_extension(&path);
    let line_count = content.lines().count() as u32;

    let file = EditorFile {
        id: Uuid::new_v4().to_string(),
        path: path.clone(),
        content,
        encoding: "utf-8".into(),
        language: Some(language),
        modified: false,
        line_count,
        size_bytes: metadata.len(),
    };

    let mut files = state.open_files.lock().unwrap();
    files.insert(file.id.clone(), file.clone());

    Ok(file)
}

#[tauri::command]
pub fn editor_save(
    file_id: String,
    content: String,
    state: tauri::State<'_, EditorState>,
) -> Result<(), EditorError> {
    let mut files = state.open_files.lock().unwrap();
    let file = files
        .get_mut(&file_id)
        .ok_or_else(|| EditorError::FileNotFound(file_id.clone()))?;

    std::fs::write(&file.path, &content)
        .map_err(|e| EditorError::WriteError(e.to_string()))?;

    file.content = content.clone();
    file.modified = false;
    file.line_count = content.lines().count() as u32;
    file.size_bytes = content.len() as u64;

    Ok(())
}

#[tauri::command]
pub fn editor_close(
    file_id: String,
    state: tauri::State<'_, EditorState>,
) -> Result<(), EditorError> {
    let mut files = state.open_files.lock().unwrap();
    if files.remove(&file_id).is_none() {
        return Err(EditorError::FileNotFound(file_id));
    }
    Ok(())
}

#[tauri::command]
pub fn editor_list_open(
    state: tauri::State<'_, EditorState>,
) -> Result<Vec<EditorFile>, EditorError> {
    let files = state.open_files.lock().unwrap();
    Ok(files.values().cloned().collect())
}

#[tauri::command]
pub fn editor_get_content(
    file_id: String,
    state: tauri::State<'_, EditorState>,
) -> Result<String, EditorError> {
    let files = state.open_files.lock().unwrap();
    let file = files
        .get(&file_id)
        .ok_or_else(|| EditorError::FileNotFound(file_id))?;
    Ok(file.content.clone())
}

#[tauri::command]
pub fn editor_detect_language(path: String) -> Result<String, EditorError> {
    Ok(detect_language_from_extension(&path))
}

#[tauri::command]
pub fn editor_diff(
    left_path: String,
    right_path: String,
) -> Result<DiffResult, EditorError> {
    let left_content = std::fs::read_to_string(&left_path)
        .map_err(|e| EditorError::ReadError(format!("{}: {}", left_path, e)))?;
    let right_content = std::fs::read_to_string(&right_path)
        .map_err(|e| EditorError::ReadError(format!("{}: {}", right_path, e)))?;

    let left_lines: Vec<&str> = left_content.lines().collect();
    let right_lines: Vec<&str> = right_content.lines().collect();

    let hunks = compute_diff(&left_lines, &right_lines);
    let stats = compute_stats(&hunks);

    Ok(DiffResult {
        left_path,
        right_path,
        hunks,
        stats,
    })
}

#[tauri::command]
pub fn editor_diff_content(
    left: String,
    right: String,
) -> Result<DiffResult, EditorError> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    let hunks = compute_diff(&left_lines, &right_lines);
    let stats = compute_stats(&hunks);

    Ok(DiffResult {
        left_path: "<left>".into(),
        right_path: "<right>".into(),
        hunks,
        stats,
    })
}

#[tauri::command]
pub fn editor_search(
    file_id: String,
    query: String,
    regex: bool,
    state: tauri::State<'_, EditorState>,
) -> Result<Vec<SearchMatch>, EditorError> {
    let files = state.open_files.lock().unwrap();
    let file = files
        .get(&file_id)
        .ok_or_else(|| EditorError::FileNotFound(file_id))?;

    let mut matches = Vec::new();

    if regex {
        let re = regex::Regex::new(&query)
            .map_err(|e| EditorError::DiffError(format!("Invalid regex: {}", e)))?;
        for (line_idx, line) in file.content.lines().enumerate() {
            for m in re.find_iter(line) {
                matches.push(SearchMatch {
                    line: line_idx as u32 + 1,
                    column: m.start() as u32 + 1,
                    length: m.len() as u32,
                    text: m.as_str().to_string(),
                });
            }
        }
    } else {
        for (line_idx, line) in file.content.lines().enumerate() {
            let mut start = 0;
            while let Some(pos) = line[start..].find(&query) {
                let abs_pos = start + pos;
                matches.push(SearchMatch {
                    line: line_idx as u32 + 1,
                    column: abs_pos as u32 + 1,
                    length: query.len() as u32,
                    text: query.clone(),
                });
                start = abs_pos + query.len();
            }
        }
    }

    Ok(matches)
}

#[tauri::command]
pub fn editor_replace(
    file_id: String,
    query: String,
    replacement: String,
    regex: bool,
    all: bool,
    state: tauri::State<'_, EditorState>,
) -> Result<u32, EditorError> {
    let mut files = state.open_files.lock().unwrap();
    let file = files
        .get_mut(&file_id)
        .ok_or_else(|| EditorError::FileNotFound(file_id))?;

    let (new_content, count) = if regex {
        let re = regex::Regex::new(&query)
            .map_err(|e| EditorError::DiffError(format!("Invalid regex: {}", e)))?;
        if all {
            let count = re.find_iter(&file.content).count() as u32;
            let new = re.replace_all(&file.content, replacement.as_str()).to_string();
            (new, count)
        } else {
            let count = if re.is_match(&file.content) { 1u32 } else { 0 };
            let new = re.replace(&file.content, replacement.as_str()).to_string();
            (new, count)
        }
    } else if all {
        let count = file.content.matches(&query).count() as u32;
        let new = file.content.replace(&query, &replacement);
        (new, count)
    } else {
        if let Some(_) = file.content.find(&query) {
            let new = file.content.replacen(&query, &replacement, 1);
            (new, 1)
        } else {
            (file.content.clone(), 0)
        }
    };

    file.content = new_content;
    file.modified = true;
    file.line_count = file.content.lines().count() as u32;

    Ok(count)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_file_lifecycle() {
        let state = EditorState::new();

        // Create a temp file
        let dir = std::env::temp_dir().join("crossterm-editor-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        // Open
        let path_str = file_path.to_string_lossy().to_string();
        let file = {
            let content = std::fs::read_to_string(&file_path).unwrap();
            let metadata = std::fs::metadata(&file_path).unwrap();
            let f = EditorFile {
                id: Uuid::new_v4().to_string(),
                path: path_str.clone(),
                content: content.clone(),
                encoding: "utf-8".into(),
                language: Some(detect_language_from_extension(&path_str)),
                modified: false,
                line_count: content.lines().count() as u32,
                size_bytes: metadata.len(),
            };
            let mut files = state.open_files.lock().unwrap();
            files.insert(f.id.clone(), f.clone());
            f
        };

        assert_eq!(file.language.as_deref(), Some("rust"));
        assert_eq!(file.line_count, 3);
        assert!(!file.modified);

        // Read content
        {
            let files = state.open_files.lock().unwrap();
            let f = files.get(&file.id).unwrap();
            assert!(f.content.contains("fn main()"));
        }

        // Save with new content
        let new_content = "fn main() {\n    println!(\"updated\");\n}\n";
        {
            std::fs::write(&file_path, new_content).unwrap();
            let mut files = state.open_files.lock().unwrap();
            let f = files.get_mut(&file.id).unwrap();
            f.content = new_content.to_string();
            f.modified = false;
            f.line_count = new_content.lines().count() as u32;
        }

        // Close
        {
            let mut files = state.open_files.lock().unwrap();
            let removed = files.remove(&file.id);
            assert!(removed.is_some());
            assert!(files.is_empty());
        }

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_diff_content() {
        let left = "line1\nline2\nline3\nline4";
        let right = "line1\nmodified\nline3\nline4\nline5";

        let left_lines: Vec<&str> = left.lines().collect();
        let right_lines: Vec<&str> = right.lines().collect();

        let hunks = compute_diff(&left_lines, &right_lines);
        let stats = compute_stats(&hunks);

        // Should detect changes
        assert!(!hunks.is_empty());
        // line2 -> modified = 1 removal + 1 addition
        // line5 added = 1 addition
        assert!(stats.additions > 0);
        assert!(stats.deletions > 0);

        // Verify line types
        let all_lines: Vec<&DiffLine> = hunks.iter().flat_map(|h| h.lines.iter()).collect();
        let added = all_lines
            .iter()
            .filter(|l| matches!(l.line_type, DiffLineType::Added))
            .count();
        let removed = all_lines
            .iter()
            .filter(|l| matches!(l.line_type, DiffLineType::Removed))
            .count();
        assert!(added > 0, "Should have additions");
        assert!(removed > 0, "Should have removals");
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language_from_extension("main.rs"), "rust");
        assert_eq!(detect_language_from_extension("app.tsx"), "typescriptreact");
        assert_eq!(detect_language_from_extension("style.css"), "css");
        assert_eq!(detect_language_from_extension("config.json"), "json");
        assert_eq!(detect_language_from_extension("readme.md"), "markdown");
        assert_eq!(detect_language_from_extension("script.py"), "python");
        assert_eq!(detect_language_from_extension("server.go"), "go");
        assert_eq!(detect_language_from_extension("noext"), "plaintext");
        assert_eq!(detect_language_from_extension("data.yaml"), "yaml");
        assert_eq!(detect_language_from_extension("build.sh"), "shell");
    }

    #[test]
    fn test_search_in_content() {
        let state = EditorState::new();

        let content = "Hello World\nfoo bar baz\nHello again\ntest line";
        let file = EditorFile {
            id: Uuid::new_v4().to_string(),
            path: "test.txt".into(),
            content: content.into(),
            encoding: "utf-8".into(),
            language: Some("plaintext".into()),
            modified: false,
            line_count: 4,
            size_bytes: content.len() as u64,
        };
        let file_id = file.id.clone();

        {
            let mut files = state.open_files.lock().unwrap();
            files.insert(file.id.clone(), file);
        }

        // Plain text search
        {
            let files = state.open_files.lock().unwrap();
            let f = files.get(&file_id).unwrap();
            let mut matches = Vec::new();
            let query = "Hello";
            for (line_idx, line) in f.content.lines().enumerate() {
                let mut start = 0;
                while let Some(pos) = line[start..].find(query) {
                    let abs_pos = start + pos;
                    matches.push(SearchMatch {
                        line: line_idx as u32 + 1,
                        column: abs_pos as u32 + 1,
                        length: query.len() as u32,
                        text: query.to_string(),
                    });
                    start = abs_pos + query.len();
                }
            }
            assert_eq!(matches.len(), 2);
            assert_eq!(matches[0].line, 1);
            assert_eq!(matches[1].line, 3);
        }

        // Regex search
        {
            let files = state.open_files.lock().unwrap();
            let f = files.get(&file_id).unwrap();
            let re = regex::Regex::new(r"Hello \w+").unwrap();
            let mut matches = Vec::new();
            for (line_idx, line) in f.content.lines().enumerate() {
                for m in re.find_iter(line) {
                    matches.push(SearchMatch {
                        line: line_idx as u32 + 1,
                        column: m.start() as u32 + 1,
                        length: m.len() as u32,
                        text: m.as_str().to_string(),
                    });
                }
            }
            assert_eq!(matches.len(), 2);
            assert_eq!(matches[0].text, "Hello World");
            assert_eq!(matches[1].text, "Hello again");
        }
    }
}
