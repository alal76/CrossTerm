use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
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
}
