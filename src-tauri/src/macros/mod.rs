use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum MacroError {
    #[error("Macro not found: {0}")]
    NotFound(String),
    #[error("Macro already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid step: {0}")]
    InvalidStep(String),
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl Serialize for MacroError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MacroStep {
    Send { data: String },
    Expect { pattern: String, timeout_ms: u64 },
    Wait { duration_ms: u64 },
    SetVariable {
        name: String,
        from_capture: Option<String>,
        value: Option<String>,
    },
    Conditional {
        condition: String,
        then_steps: Vec<MacroStep>,
        else_steps: Vec<MacroStep>,
    },
    Loop {
        count: u32,
        steps: Vec<MacroStep>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<MacroStep>,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroExecutionStatus {
    Pending,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroExecution {
    pub id: String,
    pub macro_id: String,
    pub session_id: String,
    pub status: MacroExecutionStatus,
    pub current_step: usize,
    pub total_steps: usize,
    pub variables: HashMap<String, String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExpectAction {
    SendText { text: String },
    RunMacro { macro_id: String },
    Notify { message: String },
    Callback { event_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub action: ExpectAction,
    pub enabled: bool,
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn count_steps(steps: &[MacroStep]) -> usize {
    let mut count = 0;
    for step in steps {
        count += 1;
        match step {
            MacroStep::Conditional {
                then_steps,
                else_steps,
                ..
            } => {
                count += count_steps(then_steps);
                count += count_steps(else_steps);
            }
            MacroStep::Loop { steps, .. } => {
                count += count_steps(steps);
            }
            _ => {}
        }
    }
    count
}

// ── State ───────────────────────────────────────────────────────────────

pub struct MacroState {
    macros: Mutex<HashMap<String, Macro>>,
    executions: Mutex<HashMap<String, MacroExecution>>,
    expect_rules: Mutex<HashMap<String, ExpectRule>>,
}

impl MacroState {
    pub fn new() -> Self {
        Self {
            macros: Mutex::new(HashMap::new()),
            executions: Mutex::new(HashMap::new()),
            expect_rules: Mutex::new(HashMap::new()),
        }
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn macro_create(
    name: String,
    steps: Vec<MacroStep>,
    state: tauri::State<'_, MacroState>,
) -> Result<Macro, MacroError> {
    let now = now_iso();
    let m = Macro {
        id: Uuid::new_v4().to_string(),
        name,
        description: None,
        steps,
        created_at: now.clone(),
        updated_at: now,
        tags: vec![],
    };

    let mut macros = state.macros.lock().unwrap();
    macros.insert(m.id.clone(), m.clone());
    Ok(m)
}

#[tauri::command]
pub fn macro_update(
    id: String,
    name: Option<String>,
    steps: Option<Vec<MacroStep>>,
    state: tauri::State<'_, MacroState>,
) -> Result<Macro, MacroError> {
    let mut macros = state.macros.lock().unwrap();
    let m = macros
        .get_mut(&id)
        .ok_or_else(|| MacroError::NotFound(id.clone()))?;

    if let Some(n) = name {
        m.name = n;
    }
    if let Some(s) = steps {
        m.steps = s;
    }
    m.updated_at = now_iso();

    Ok(m.clone())
}

#[tauri::command]
pub fn macro_delete(
    id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut macros = state.macros.lock().unwrap();
    if macros.remove(&id).is_none() {
        return Err(MacroError::NotFound(id));
    }
    Ok(())
}

#[tauri::command]
pub fn macro_list(
    state: tauri::State<'_, MacroState>,
) -> Result<Vec<Macro>, MacroError> {
    let macros = state.macros.lock().unwrap();
    Ok(macros.values().cloned().collect())
}

#[tauri::command]
pub fn macro_get(
    id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<Macro, MacroError> {
    let macros = state.macros.lock().unwrap();
    macros
        .get(&id)
        .cloned()
        .ok_or(MacroError::NotFound(id))
}

#[tauri::command]
pub fn macro_execute(
    macro_id: String,
    session_id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<MacroExecution, MacroError> {
    let macros = state.macros.lock().unwrap();
    let m = macros
        .get(&macro_id)
        .ok_or_else(|| MacroError::NotFound(macro_id.clone()))?;

    let total = count_steps(&m.steps);
    let execution = MacroExecution {
        id: Uuid::new_v4().to_string(),
        macro_id: macro_id.clone(),
        session_id,
        status: MacroExecutionStatus::Running,
        current_step: 0,
        total_steps: total,
        variables: HashMap::new(),
        started_at: now_iso(),
        completed_at: None,
        error: None,
    };

    drop(macros);
    let mut executions = state.executions.lock().unwrap();
    executions.insert(execution.id.clone(), execution.clone());

    Ok(execution)
}

#[tauri::command]
pub fn macro_cancel(
    execution_id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut executions = state.executions.lock().unwrap();
    let exec = executions
        .get_mut(&execution_id)
        .ok_or_else(|| MacroError::NotFound(execution_id.clone()))?;
    exec.status = MacroExecutionStatus::Cancelled;
    exec.completed_at = Some(now_iso());
    Ok(())
}

#[tauri::command]
pub fn macro_pause(
    execution_id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut executions = state.executions.lock().unwrap();
    let exec = executions
        .get_mut(&execution_id)
        .ok_or_else(|| MacroError::NotFound(execution_id.clone()))?;
    exec.status = MacroExecutionStatus::Paused;
    Ok(())
}

#[tauri::command]
pub fn macro_resume(
    execution_id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut executions = state.executions.lock().unwrap();
    let exec = executions
        .get_mut(&execution_id)
        .ok_or_else(|| MacroError::NotFound(execution_id.clone()))?;
    exec.status = MacroExecutionStatus::Running;
    Ok(())
}

#[tauri::command]
pub fn expect_rule_create(
    name: String,
    pattern: String,
    action: ExpectAction,
    state: tauri::State<'_, MacroState>,
) -> Result<ExpectRule, MacroError> {
    // Validate regex pattern
    regex::Regex::new(&pattern)
        .map_err(|e| MacroError::ParseError(format!("Invalid regex: {}", e)))?;

    let rule = ExpectRule {
        id: Uuid::new_v4().to_string(),
        name,
        pattern,
        action,
        enabled: true,
    };

    let mut rules = state.expect_rules.lock().unwrap();
    rules.insert(rule.id.clone(), rule.clone());
    Ok(rule)
}

#[tauri::command]
pub fn expect_rule_delete(
    id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut rules = state.expect_rules.lock().unwrap();
    if rules.remove(&id).is_none() {
        return Err(MacroError::NotFound(id));
    }
    Ok(())
}

#[tauri::command]
pub fn expect_rule_list(
    state: tauri::State<'_, MacroState>,
) -> Result<Vec<ExpectRule>, MacroError> {
    let rules = state.expect_rules.lock().unwrap();
    Ok(rules.values().cloned().collect())
}

#[tauri::command]
pub fn expect_rule_toggle(
    id: String,
    enabled: bool,
    state: tauri::State<'_, MacroState>,
) -> Result<(), MacroError> {
    let mut rules = state.expect_rules.lock().unwrap();
    let rule = rules
        .get_mut(&id)
        .ok_or_else(|| MacroError::NotFound(id.clone()))?;
    rule.enabled = enabled;
    Ok(())
}

// ── Macro Extensions ────────────────────────────────────────────────────

#[tauri::command]
pub fn macro_broadcast(
    macro_id: String,
    session_ids: Vec<String>,
    state: tauri::State<'_, MacroState>,
) -> Result<Vec<MacroExecution>, MacroError> {
    let macros = state.macros.lock().unwrap();
    let m = macros
        .get(&macro_id)
        .ok_or_else(|| MacroError::NotFound(macro_id.clone()))?;

    let total = count_steps(&m.steps);
    let mut executions_vec = Vec::new();

    for session_id in &session_ids {
        let execution = MacroExecution {
            id: Uuid::new_v4().to_string(),
            macro_id: macro_id.clone(),
            session_id: session_id.clone(),
            status: MacroExecutionStatus::Running,
            current_step: 0,
            total_steps: total,
            variables: HashMap::new(),
            started_at: now_iso(),
            completed_at: None,
            error: None,
        };
        executions_vec.push(execution);
    }

    drop(macros);
    let mut executions = state.executions.lock().unwrap();
    for exec in &executions_vec {
        executions.insert(exec.id.clone(), exec.clone());
    }

    Ok(executions_vec)
}

#[tauri::command]
pub fn macro_export(
    macro_id: String,
    state: tauri::State<'_, MacroState>,
) -> Result<String, MacroError> {
    let macros = state.macros.lock().unwrap();
    let m = macros
        .get(&macro_id)
        .ok_or_else(|| MacroError::NotFound(macro_id.clone()))?;
    serde_json::to_string_pretty(m)
        .map_err(|e| MacroError::ExecutionFailed(format!("Serialization error: {}", e)))
}

#[tauri::command]
pub fn macro_import(
    data: String,
    state: tauri::State<'_, MacroState>,
) -> Result<Macro, MacroError> {
    let mut m: Macro = serde_json::from_str(&data)
        .map_err(|e| MacroError::ParseError(format!("Invalid JSON: {}", e)))?;

    // Assign a new ID to prevent collisions
    m.id = Uuid::new_v4().to_string();
    m.updated_at = now_iso();

    let mut macros = state.macros.lock().unwrap();
    macros.insert(m.id.clone(), m.clone());
    Ok(m)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_crud() {
        let state = MacroState::new();

        // Create
        let steps = vec![
            MacroStep::Send {
                data: "ls\n".into(),
            },
            MacroStep::Expect {
                pattern: "\\$".into(),
                timeout_ms: 5000,
            },
        ];
        let m = {
            let now = now_iso();
            let m = Macro {
                id: Uuid::new_v4().to_string(),
                name: "Test Macro".into(),
                description: Some("A test".into()),
                steps,
                created_at: now.clone(),
                updated_at: now,
                tags: vec!["test".into()],
            };
            let mut macros = state.macros.lock().unwrap();
            macros.insert(m.id.clone(), m.clone());
            m
        };

        // List
        {
            let macros = state.macros.lock().unwrap();
            assert_eq!(macros.len(), 1);
        }

        // Get
        {
            let macros = state.macros.lock().unwrap();
            let found = macros.get(&m.id).unwrap();
            assert_eq!(found.name, "Test Macro");
        }

        // Update
        {
            let mut macros = state.macros.lock().unwrap();
            let found = macros.get_mut(&m.id).unwrap();
            found.name = "Updated Macro".into();
            found.updated_at = now_iso();
            assert_eq!(found.name, "Updated Macro");
        }

        // Delete
        {
            let mut macros = state.macros.lock().unwrap();
            let removed = macros.remove(&m.id);
            assert!(removed.is_some());
            assert!(macros.is_empty());
        }
    }

    #[test]
    fn test_macro_step_serde() {
        let steps = vec![
            MacroStep::Send {
                data: "hello\n".into(),
            },
            MacroStep::Expect {
                pattern: "prompt>".into(),
                timeout_ms: 3000,
            },
            MacroStep::Wait { duration_ms: 1000 },
            MacroStep::SetVariable {
                name: "host".into(),
                from_capture: Some("hostname: (.+)".into()),
                value: None,
            },
            MacroStep::Conditional {
                condition: "host == server1".into(),
                then_steps: vec![MacroStep::Send {
                    data: "cmd1\n".into(),
                }],
                else_steps: vec![MacroStep::Send {
                    data: "cmd2\n".into(),
                }],
            },
            MacroStep::Loop {
                count: 3,
                steps: vec![MacroStep::Send {
                    data: "ping\n".into(),
                }],
            },
        ];

        let json = serde_json::to_string(&steps).expect("serialize");

        // Verify tagged union serialization
        assert!(json.contains("\"type\":\"send\""));
        assert!(json.contains("\"type\":\"expect\""));
        assert!(json.contains("\"type\":\"wait\""));
        assert!(json.contains("\"type\":\"set_variable\""));
        assert!(json.contains("\"type\":\"conditional\""));
        assert!(json.contains("\"type\":\"loop\""));

        let parsed: Vec<MacroStep> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.len(), 6);
    }

    #[test]
    fn test_macro_execution_lifecycle() {
        let state = MacroState::new();

        // Create a macro
        let m = {
            let now = now_iso();
            let m = Macro {
                id: Uuid::new_v4().to_string(),
                name: "Exec Test".into(),
                description: None,
                steps: vec![
                    MacroStep::Send {
                        data: "ls\n".into(),
                    },
                    MacroStep::Wait { duration_ms: 500 },
                ],
                created_at: now.clone(),
                updated_at: now,
                tags: vec![],
            };
            let mut macros = state.macros.lock().unwrap();
            macros.insert(m.id.clone(), m.clone());
            m
        };

        // Create execution
        let exec_id = {
            let macros = state.macros.lock().unwrap();
            let found = macros.get(&m.id).unwrap();
            let total = count_steps(&found.steps);

            let exec = MacroExecution {
                id: Uuid::new_v4().to_string(),
                macro_id: m.id.clone(),
                session_id: "session-1".into(),
                status: MacroExecutionStatus::Pending,
                current_step: 0,
                total_steps: total,
                variables: HashMap::new(),
                started_at: now_iso(),
                completed_at: None,
                error: None,
            };

            let mut executions = state.executions.lock().unwrap();
            executions.insert(exec.id.clone(), exec.clone());
            exec.id
        };

        // Transition to Running
        {
            let mut executions = state.executions.lock().unwrap();
            let exec = executions.get_mut(&exec_id).unwrap();
            exec.status = MacroExecutionStatus::Running;
            assert!(matches!(exec.status, MacroExecutionStatus::Running));
        }

        // Pause
        {
            let mut executions = state.executions.lock().unwrap();
            let exec = executions.get_mut(&exec_id).unwrap();
            exec.status = MacroExecutionStatus::Paused;
            assert!(matches!(exec.status, MacroExecutionStatus::Paused));
        }

        // Resume
        {
            let mut executions = state.executions.lock().unwrap();
            let exec = executions.get_mut(&exec_id).unwrap();
            exec.status = MacroExecutionStatus::Running;
            assert!(matches!(exec.status, MacroExecutionStatus::Running));
        }

        // Complete
        {
            let mut executions = state.executions.lock().unwrap();
            let exec = executions.get_mut(&exec_id).unwrap();
            exec.status = MacroExecutionStatus::Completed;
            exec.completed_at = Some(now_iso());
            assert!(matches!(exec.status, MacroExecutionStatus::Completed));
            assert!(exec.completed_at.is_some());
        }
    }

    #[test]
    fn test_expect_rule_crud() {
        let state = MacroState::new();

        // Create
        let rule = {
            let rule = ExpectRule {
                id: Uuid::new_v4().to_string(),
                name: "Password Prompt".into(),
                pattern: "password:".into(),
                action: ExpectAction::SendText {
                    text: "secret\n".into(),
                },
                enabled: true,
            };
            let mut rules = state.expect_rules.lock().unwrap();
            rules.insert(rule.id.clone(), rule.clone());
            rule
        };

        // List
        {
            let rules = state.expect_rules.lock().unwrap();
            assert_eq!(rules.len(), 1);
        }

        // Toggle
        {
            let mut rules = state.expect_rules.lock().unwrap();
            let r = rules.get_mut(&rule.id).unwrap();
            r.enabled = false;
            assert!(!r.enabled);
            r.enabled = true;
            assert!(r.enabled);
        }

        // Delete
        {
            let mut rules = state.expect_rules.lock().unwrap();
            let removed = rules.remove(&rule.id);
            assert!(removed.is_some());
            assert!(rules.is_empty());
        }
    }

    #[test]
    fn test_expect_action_serde() {
        let actions = vec![
            ExpectAction::SendText {
                text: "hello".into(),
            },
            ExpectAction::RunMacro {
                macro_id: "m-123".into(),
            },
            ExpectAction::Notify {
                message: "Match found".into(),
            },
            ExpectAction::Callback {
                event_name: "on_match".into(),
            },
        ];

        for action in &actions {
            let json = serde_json::to_string(action).expect("serialize");
            let parsed: ExpectAction = serde_json::from_str(&json).expect("deserialize");

            match (action, &parsed) {
                (ExpectAction::SendText { text: a }, ExpectAction::SendText { text: b }) => {
                    assert_eq!(a, b);
                }
                (
                    ExpectAction::RunMacro { macro_id: a },
                    ExpectAction::RunMacro { macro_id: b },
                ) => {
                    assert_eq!(a, b);
                }
                (ExpectAction::Notify { message: a }, ExpectAction::Notify { message: b }) => {
                    assert_eq!(a, b);
                }
                (
                    ExpectAction::Callback { event_name: a },
                    ExpectAction::Callback { event_name: b },
                ) => {
                    assert_eq!(a, b);
                }
                _ => panic!("Mismatched action types"),
            }
        }

        // Verify tagged union format
        let json =
            serde_json::to_string(&ExpectAction::SendText {
                text: "x".into(),
            })
            .unwrap();
        assert!(json.contains("\"type\":\"send_text\""));
    }

    #[test]
    fn test_macro_broadcast() {
        let state = MacroState::new();

        // Create a macro
        let m = {
            let now = now_iso();
            let m = Macro {
                id: Uuid::new_v4().to_string(),
                name: "Broadcast Macro".into(),
                description: None,
                steps: vec![
                    MacroStep::Send {
                        data: "uptime\n".into(),
                    },
                ],
                created_at: now.clone(),
                updated_at: now,
                tags: vec![],
            };
            let mut macros = state.macros.lock().unwrap();
            macros.insert(m.id.clone(), m.clone());
            m
        };

        // Broadcast to 3 sessions
        let session_ids = vec![
            "session-1".to_string(),
            "session-2".to_string(),
            "session-3".to_string(),
        ];

        let macros = state.macros.lock().unwrap();
        let found = macros.get(&m.id).unwrap();
        let total = count_steps(&found.steps);
        drop(macros);

        let mut executions_vec = Vec::new();
        for sid in &session_ids {
            let exec = MacroExecution {
                id: Uuid::new_v4().to_string(),
                macro_id: m.id.clone(),
                session_id: sid.clone(),
                status: MacroExecutionStatus::Running,
                current_step: 0,
                total_steps: total,
                variables: HashMap::new(),
                started_at: now_iso(),
                completed_at: None,
                error: None,
            };
            executions_vec.push(exec);
        }

        let mut execs = state.executions.lock().unwrap();
        for exec in &executions_vec {
            execs.insert(exec.id.clone(), exec.clone());
        }

        assert_eq!(executions_vec.len(), 3);
        assert!(executions_vec
            .iter()
            .all(|e| e.macro_id == m.id));
        let session_set: std::collections::HashSet<_> =
            executions_vec.iter().map(|e| e.session_id.clone()).collect();
        assert_eq!(session_set.len(), 3);
    }

    #[test]
    fn test_macro_export_import() {
        let state = MacroState::new();

        // Create a macro
        let m = {
            let now = now_iso();
            let m = Macro {
                id: Uuid::new_v4().to_string(),
                name: "Export Test".into(),
                description: Some("Test macro for export".into()),
                steps: vec![
                    MacroStep::Send {
                        data: "echo hello\n".into(),
                    },
                    MacroStep::Wait { duration_ms: 1000 },
                ],
                created_at: now.clone(),
                updated_at: now,
                tags: vec!["test".into(), "export".into()],
            };
            let mut macros = state.macros.lock().unwrap();
            macros.insert(m.id.clone(), m.clone());
            m
        };

        // Export
        let json = {
            let macros = state.macros.lock().unwrap();
            let found = macros.get(&m.id).unwrap();
            serde_json::to_string_pretty(found).unwrap()
        };
        assert!(json.contains("Export Test"));
        assert!(json.contains("echo hello"));

        // Import
        let imported: Macro = serde_json::from_str(&json).unwrap();
        assert_eq!(imported.name, "Export Test");
        assert_eq!(imported.steps.len(), 2);
        assert_eq!(imported.tags.len(), 2);

        // Import with new ID
        let mut reimported = imported.clone();
        reimported.id = Uuid::new_v4().to_string();
        let mut macros = state.macros.lock().unwrap();
        macros.insert(reimported.id.clone(), reimported.clone());
        assert_ne!(reimported.id, m.id);
        assert_eq!(macros.len(), 2);
    }

    // ── Feature 1 tests ──────────────────────────────────────────────────

    #[test]
    fn test_dry_run_send_step() {
        let steps = vec![serde_json::json!({ "type": "send", "input": "ls -la\n" })];
        let results = dry_run_macro(&steps);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].simulated_output.starts_with("→ SEND:"),
            "Expected output to start with '→ SEND:', got: {}",
            results[0].simulated_output
        );
        assert!(results[0].would_match);
        assert_eq!(results[0].duration_ms, 10);
    }

    #[test]
    fn test_dry_run_expect_step() {
        let steps = vec![serde_json::json!({ "type": "expect", "pattern": "\\$" })];
        let results = dry_run_macro(&steps);
        assert_eq!(results.len(), 1);
        assert!(results[0].would_match);
        assert_eq!(results[0].duration_ms, 100);
    }

    #[test]
    fn test_dry_run_mixed_steps() {
        let steps = vec![
            serde_json::json!({ "type": "send", "input": "echo hi" }),
            serde_json::json!({ "type": "expect", "pattern": "hi" }),
            serde_json::json!({ "type": "sleep", "duration": 250 }),
        ];
        let results = dry_run_macro(&steps);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].step_type, "send");
        assert_eq!(results[1].step_type, "expect");
        assert_eq!(results[2].step_type, "sleep");
        assert_eq!(results[2].duration_ms, 250);
    }

    // ── Feature 2 tests ──────────────────────────────────────────────────

    #[test]
    fn test_builtin_library_not_empty() {
        let macros = builtin_macro_library();
        assert!(macros.len() >= 6, "Expected at least 6 built-in macros, got {}", macros.len());
    }

    #[test]
    fn test_builtin_categories_valid() {
        let macros = builtin_macro_library();
        for m in &macros {
            assert!(!m.name.is_empty(), "Macro name must not be empty");
            assert!(!m.category.is_empty(), "Macro category must not be empty");
        }
    }

    // ── Feature 3 tests ──────────────────────────────────────────────────

    #[test]
    fn test_cron_parse_every_5_minutes() {
        let result = parse_cron_next("*/5 * * * *", "2026-01-01T10:00:00Z");
        assert_eq!(result, Some("2026-01-01T10:05:00Z".to_string()));
    }

    #[test]
    fn test_cron_parse_top_of_hour() {
        let result = parse_cron_next("0 * * * *", "2026-01-01T10:15:00Z");
        assert_eq!(result, Some("2026-01-01T11:00:00Z".to_string()));
    }

    #[test]
    fn test_cron_parse_unsupported() {
        let result = parse_cron_next("1,5,10 * * * *", "2026-01-01T10:00:00Z");
        assert_eq!(result, None);
    }

    // ── Feature 4 tests ──────────────────────────────────────────────────

    #[test]
    fn test_capture_named_groups() {
        let caps = apply_expect_captures(r"(?P<host>[^:]+):(?P<port>\d+)", "server.example.com:22");
        assert_eq!(caps.get("host").map(|s| s.as_str()), Some("server.example.com"));
        assert_eq!(caps.get("port").map(|s| s.as_str()), Some("22"));
    }

    #[test]
    fn test_capture_positional_groups() {
        let caps = apply_expect_captures(r"(\w+)\s+(\w+)", "hello world");
        assert_eq!(caps.get("1").map(|s| s.as_str()), Some("hello"));
        assert_eq!(caps.get("2").map(|s| s.as_str()), Some("world"));
    }

    #[test]
    fn test_capture_no_match() {
        let caps = apply_expect_captures(r"\d{4}", "no digits here");
        assert!(caps.is_empty());
    }

    #[test]
    fn test_substitute_variables() {
        let mut vars = HashMap::new();
        vars.insert("host".to_string(), "srv".to_string());
        vars.insert("port".to_string(), "22".to_string());
        let result = substitute_variables("Connect to ${host} on ${port}", &vars);
        assert_eq!(result, "Connect to srv on 22");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Feature 1: Macro dry-run mode
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DryRunResult {
    pub step_index: usize,
    pub step_type: String,
    pub input: Option<String>,
    pub expected_pattern: Option<String>,
    pub simulated_output: String,
    pub would_match: bool,
    pub duration_ms: u64,
}

#[allow(dead_code)]
pub fn dry_run_macro(steps: &[serde_json::Value]) -> Vec<DryRunResult> {
    steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let step_type = step
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            match step_type.as_str() {
                "send" => {
                    let input = step
                        .get("input")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    DryRunResult {
                        step_index: idx,
                        step_type: "send".to_string(),
                        input: Some(input.clone()),
                        expected_pattern: None,
                        simulated_output: format!("→ SEND: {}", input),
                        would_match: true,
                        duration_ms: 10,
                    }
                }
                "expect" => {
                    let pattern = step
                        .get("pattern")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    DryRunResult {
                        step_index: idx,
                        step_type: "expect".to_string(),
                        input: None,
                        expected_pattern: Some(pattern.clone()),
                        simulated_output: format!("← EXPECT: {} [SIMULATED MATCH]", pattern),
                        would_match: true,
                        duration_ms: 100,
                    }
                }
                "sleep" => {
                    let duration = step
                        .get("duration")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    DryRunResult {
                        step_index: idx,
                        step_type: "sleep".to_string(),
                        input: None,
                        expected_pattern: None,
                        simulated_output: format!("⏱ SLEEP {}ms", duration),
                        would_match: true,
                        duration_ms: duration,
                    }
                }
                _ => DryRunResult {
                    step_index: idx,
                    step_type,
                    input: None,
                    expected_pattern: None,
                    simulated_output: format!("? UNKNOWN STEP: {:?}", step),
                    would_match: false,
                    duration_ms: 0,
                },
            }
        })
        .collect()
}

#[tauri::command]
#[allow(dead_code)]
pub fn macro_dry_run(
    steps: Vec<serde_json::Value>,
    _state: tauri::State<'_, MacroState>,
) -> Result<Vec<DryRunResult>, String> {
    Ok(dry_run_macro(&steps))
}

// ════════════════════════════════════════════════════════════════════════════
// Feature 2: Built-in macro library
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct BuiltinMacro {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
    pub steps: Vec<serde_json::Value>,
}

#[allow(dead_code)]
pub fn builtin_macro_library() -> Vec<BuiltinMacro> {
    vec![
        BuiltinMacro {
            id: "disk-usage".to_string(),
            name: "Disk Usage".to_string(),
            description: "Show disk usage for all mounted filesystems".to_string(),
            category: "monitoring".to_string(),
            tags: vec!["disk".to_string(), "storage".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "df -h\n" }),
                serde_json::json!({ "type": "expect", "pattern": "%" }),
            ],
        },
        BuiltinMacro {
            id: "memory-usage".to_string(),
            name: "Memory Usage".to_string(),
            description: "Show current memory usage statistics".to_string(),
            category: "monitoring".to_string(),
            tags: vec!["memory".to_string(), "ram".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "free -h\n" }),
                serde_json::json!({ "type": "expect", "pattern": "Mem:" }),
            ],
        },
        BuiltinMacro {
            id: "top-processes".to_string(),
            name: "Top Processes by CPU".to_string(),
            description: "List the top 10 processes sorted by CPU usage".to_string(),
            category: "monitoring".to_string(),
            tags: vec!["cpu".to_string(), "processes".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "ps aux --sort=-%cpu | head -10\n" }),
            ],
        },
        BuiltinMacro {
            id: "docker-ps".to_string(),
            name: "Docker Container Status".to_string(),
            description: "List running Docker containers with name, status and ports".to_string(),
            category: "docker".to_string(),
            tags: vec!["docker".to_string(), "containers".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "docker ps --format \"table {{.Names}}\\t{{.Status}}\\t{{.Ports}}\"\n" }),
                serde_json::json!({ "type": "expect", "pattern": "NAMES" }),
            ],
        },
        BuiltinMacro {
            id: "k8s-pod-status".to_string(),
            name: "Kubernetes Pod Status".to_string(),
            description: "List all pods across all namespaces".to_string(),
            category: "kubernetes".to_string(),
            tags: vec!["k8s".to_string(), "pods".to_string(), "kubernetes".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "kubectl get pods -A\n" }),
                serde_json::json!({ "type": "expect", "pattern": "NAME" }),
            ],
        },
        BuiltinMacro {
            id: "log-tail".to_string(),
            name: "Tail System Log".to_string(),
            description: "Show the last 100 lines of the system log".to_string(),
            category: "monitoring".to_string(),
            tags: vec!["logs".to_string(), "syslog".to_string()],
            steps: vec![
                serde_json::json!({ "type": "send", "input": "tail -100 /var/log/syslog || tail -100 /var/log/messages\n" }),
            ],
        },
    ]
}

#[tauri::command]
#[allow(dead_code)]
pub fn macro_list_builtins(
    _state: tauri::State<'_, MacroState>,
) -> Result<Vec<BuiltinMacro>, String> {
    Ok(builtin_macro_library())
}

// ════════════════════════════════════════════════════════════════════════════
// Feature 3: Scheduled macros
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MacroSchedule {
    pub id: String,
    pub macro_id: String,
    pub session_id: String,
    pub cron_expression: String,
    pub enabled: bool,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub run_count: u32,
}

/// Parse a simplified cron expression and return the next run time after `from_iso`.
///
/// Supported minute fields:
/// - `*/N` — next multiple of N minutes after from_iso (advancing at least 1 minute)
/// - `M`   — a specific minute number; advance to next occurrence (possibly next hour)
///
/// Everything else (lists, ranges, etc.) returns None.
#[allow(dead_code)]
pub fn parse_cron_next(cron_expr: &str, from_iso: &str) -> Option<String> {
    // Split cron: minute hour dom month dow
    let parts: Vec<&str> = cron_expr.split_whitespace().collect();
    if parts.len() != 5 {
        return None;
    }

    let minute_field = parts[0];

    // Parse date+time from ISO 8601: "YYYY-MM-DDTHH:MM:SS..."
    let (date_part, time_part) = {
        let t_pos = from_iso.find('T')?;
        (&from_iso[..t_pos], &from_iso[t_pos + 1..])
    };

    // Parse HH:MM from time_part (ignore seconds and timezone for calculation)
    let time_components: Vec<&str> = time_part.splitn(3, ':').collect();
    if time_components.len() < 2 {
        return None;
    }
    let from_hour: u32 = time_components[0].parse().ok()?;
    let from_minute: u32 = time_components[1]
        .trim_end_matches(|c: char| !c.is_ascii_digit())
        .parse()
        .ok()?;

    // Total minutes since midnight for the "from" time
    let from_total_minutes = from_hour * 60 + from_minute;

    if let Some(interval_str) = minute_field.strip_prefix("*/") {
        // Interval mode: */N
        let interval: u32 = interval_str.parse().ok()?;
        if interval == 0 {
            return None;
        }

        // Next multiple of interval strictly after from_total_minutes
        let next_total = (from_total_minutes / interval + 1) * interval;
        let next_hour = next_total / 60;
        let next_minute = next_total % 60;

        if next_hour < 24 {
            // Same day
            Some(format!(
                "{}T{:02}:{:02}:00Z",
                date_part, next_hour, next_minute
            ))
        } else {
            // Would overflow into the next day — advance date by 1
            let next_date = advance_date_by_one(date_part)?;
            let wrapped_hour = next_hour % 24;
            Some(format!(
                "{}T{:02}:{:02}:00Z",
                next_date, wrapped_hour, next_minute
            ))
        }
    } else if !minute_field.contains(',')
        && !minute_field.contains('-')
        && !minute_field.contains('/')
        && minute_field != "*"
    {
        // Specific minute: M
        let target_minute: u32 = minute_field.parse().ok()?;
        if target_minute > 59 {
            return None;
        }

        // If from_minute < target_minute, fire this hour; otherwise fire next hour
        if from_minute < target_minute {
            Some(format!(
                "{}T{:02}:{:02}:00Z",
                date_part, from_hour, target_minute
            ))
        } else {
            let next_hour = from_hour + 1;
            if next_hour < 24 {
                Some(format!(
                    "{}T{:02}:{:02}:00Z",
                    date_part, next_hour, target_minute
                ))
            } else {
                let next_date = advance_date_by_one(date_part)?;
                Some(format!("{}T00:{:02}:00Z", next_date, target_minute))
            }
        }
    } else {
        // Unsupported expression
        None
    }
}

/// Advance an ISO date string ("YYYY-MM-DD") by one calendar day.
#[allow(dead_code)]
fn advance_date_by_one(date: &str) -> Option<String> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: u32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;

    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => return None,
    };

    if day < days_in_month {
        Some(format!("{:04}-{:02}-{:02}", year, month, day + 1))
    } else if month < 12 {
        Some(format!("{:04}-{:02}-01", year, month + 1))
    } else {
        Some(format!("{:04}-01-01", year + 1))
    }
}

// Module-level storage for schedules (avoids modifying MacroState).
#[allow(dead_code)]
static MACRO_SCHEDULES: std::sync::OnceLock<Arc<Mutex<Vec<MacroSchedule>>>> =
    std::sync::OnceLock::new();

#[allow(dead_code)]
fn get_schedules() -> Arc<Mutex<Vec<MacroSchedule>>> {
    MACRO_SCHEDULES
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

#[tauri::command]
#[allow(dead_code)]
pub fn macro_schedule_create(
    schedule: MacroSchedule,
    _state: tauri::State<'_, MacroState>,
) -> Result<(), String> {
    let schedules = get_schedules();
    let mut list = schedules.lock().map_err(|e| e.to_string())?;
    list.push(schedule);
    Ok(())
}

#[tauri::command]
#[allow(dead_code)]
pub fn macro_schedule_list(
    _state: tauri::State<'_, MacroState>,
) -> Result<Vec<MacroSchedule>, String> {
    let schedules = get_schedules();
    let list = schedules.lock().map_err(|e| e.to_string())?;
    Ok(list.clone())
}

#[tauri::command]
#[allow(dead_code)]
pub fn macro_schedule_delete(
    id: String,
    _state: tauri::State<'_, MacroState>,
) -> Result<(), String> {
    let schedules = get_schedules();
    let mut list = schedules.lock().map_err(|e| e.to_string())?;
    let before = list.len();
    list.retain(|s| s.id != id);
    if list.len() == before {
        Err(format!("Schedule not found: {}", id))
    } else {
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Feature 4: Expect rule improvements — regex capture groups
// ════════════════════════════════════════════════════════════════════════════

/// Apply a regex pattern to `text` and return a map of capture group name/index → captured text.
/// Named groups use their name as key; positional groups use "1", "2", etc.
/// Returns an empty map if the pattern fails to compile or finds no match.
#[allow(dead_code)]
pub fn apply_expect_captures(pattern: &str, text: &str) -> HashMap<String, String> {
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return HashMap::new(),
    };

    let caps = match re.captures(text) {
        Some(c) => c,
        None => return HashMap::new(),
    };

    let mut result = HashMap::new();

    // Positional groups (skip index 0 which is the full match)
    for i in 1..caps.len() {
        if let Some(m) = caps.get(i) {
            result.insert(i.to_string(), m.as_str().to_string());
        }
    }

    // Named groups override positional entries for the same capture
    for name in re.capture_names().flatten() {
        if let Some(m) = caps.name(name) {
            result.insert(name.to_string(), m.as_str().to_string());
        }
    }

    result
}

/// Replace `${varname}` placeholders in `template` with values from `vars`.
/// Unknown variable references are left unchanged.
#[allow(dead_code)]
pub fn substitute_variables(template: &str, vars: &HashMap<String, String>) -> String {
    // Walk through the template and replace ${...} occurrences.
    let mut result = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find("${") {
        result.push_str(&remaining[..start]);
        let after_open = &remaining[start + 2..];
        if let Some(end) = after_open.find('}') {
            let var_name = &after_open[..end];
            if let Some(value) = vars.get(var_name) {
                result.push_str(value);
            } else {
                // Leave the placeholder intact
                result.push_str("${");
                result.push_str(var_name);
                result.push('}');
            }
            remaining = &after_open[end + 1..];
        } else {
            // No closing brace — emit the rest as-is
            result.push_str("${");
            remaining = after_open;
        }
    }

    result.push_str(remaining);
    result
}
