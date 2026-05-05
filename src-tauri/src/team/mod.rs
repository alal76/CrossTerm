use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ── Team Session Library ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSession {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub protocol: String,
    pub owner_id: String,
    pub read_only_for: Vec<String>, // member IDs who can view but not edit
    pub tags: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamLibraryState {
    pub sessions: Vec<SharedSession>,
    pub last_synced: Option<String>,
}

// State storage
use std::sync::OnceLock;
static TEAM_LIBRARY: OnceLock<Arc<Mutex<TeamLibraryState>>> = OnceLock::new();
fn get_library() -> Arc<Mutex<TeamLibraryState>> {
    TEAM_LIBRARY
        .get_or_init(|| {
            Arc::new(Mutex::new(TeamLibraryState {
                sessions: Vec::new(),
                last_synced: None,
            }))
        })
        .clone()
}

/// List all shared sessions in the team library.
#[tauri::command]
pub fn team_session_list() -> Result<Vec<SharedSession>, String> {
    let lib = get_library();
    let guard = lib.lock().map_err(|e| e.to_string())?;
    Ok(guard.sessions.clone())
}

/// Publish (add or replace) a session in the team library.
#[tauri::command]
pub fn team_session_publish(session: SharedSession) -> Result<(), String> {
    let lib = get_library();
    let mut guard = lib.lock().map_err(|e| e.to_string())?;
    // Replace existing entry with same id, or push new
    if let Some(existing) = guard.sessions.iter_mut().find(|s| s.id == session.id) {
        *existing = session;
    } else {
        guard.sessions.push(session);
    }
    Ok(())
}

/// Remove a session from the team library by id.
#[tauri::command]
pub fn team_session_unpublish(session_id: String) -> Result<(), String> {
    let lib = get_library();
    let mut guard = lib.lock().map_err(|e| e.to_string())?;
    let before = guard.sessions.len();
    guard.sessions.retain(|s| s.id != session_id);
    if guard.sessions.len() == before {
        return Err(format!("Session not found: {session_id}"));
    }
    Ok(())
}

// ── Presence Indicators ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEntry {
    pub member_id: String,
    pub display_name: String,
    pub connected_host: String,
    pub connected_at: String,
    pub session_id: String,
}

static PRESENCE: OnceLock<Arc<Mutex<Vec<PresenceEntry>>>> = OnceLock::new();
fn get_presence() -> Arc<Mutex<Vec<PresenceEntry>>> {
    PRESENCE
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

/// Insert or update the presence entry for a member.
#[tauri::command]
pub fn team_presence_update(entry: PresenceEntry) -> Result<(), String> {
    let presence = get_presence();
    let mut guard = presence.lock().map_err(|e| e.to_string())?;
    if let Some(existing) = guard.iter_mut().find(|e| e.member_id == entry.member_id) {
        *existing = entry;
    } else {
        guard.push(entry);
    }
    Ok(())
}

/// Return all current presence entries.
#[tauri::command]
pub fn team_presence_list() -> Result<Vec<PresenceEntry>, String> {
    let presence = get_presence();
    let guard = presence.lock().map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

/// Remove the presence entry for the given member_id.
#[tauri::command]
pub fn team_presence_clear(member_id: String) -> Result<(), String> {
    let presence = get_presence();
    let mut guard = presence.lock().map_err(|e| e.to_string())?;
    guard.retain(|e| e.member_id != member_id);
    Ok(())
}

// ── Session Handoff ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HandoffStatus {
    Pending,
    Accepted,
    Declined,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionHandoffRequest {
    pub id: String,
    pub from_member_id: String,
    pub to_member_id: String,
    pub session_id: String,
    pub message: Option<String>,
    pub status: HandoffStatus,
    pub created_at: String,
}

static HANDOFFS: OnceLock<Arc<Mutex<Vec<SessionHandoffRequest>>>> = OnceLock::new();
fn get_handoffs() -> Arc<Mutex<Vec<SessionHandoffRequest>>> {
    HANDOFFS
        .get_or_init(|| Arc::new(Mutex::new(Vec::new())))
        .clone()
}

/// Submit a new session handoff request with status Pending.
#[tauri::command]
pub fn team_handoff_request(request: SessionHandoffRequest) -> Result<(), String> {
    let handoffs = get_handoffs();
    let mut guard = handoffs.lock().map_err(|e| e.to_string())?;
    guard.push(request);
    Ok(())
}

/// Accept or decline an existing handoff request by id.
#[tauri::command]
pub fn team_handoff_respond(request_id: String, accept: bool) -> Result<(), String> {
    let handoffs = get_handoffs();
    let mut guard = handoffs.lock().map_err(|e| e.to_string())?;
    let req = guard
        .iter_mut()
        .find(|r| r.id == request_id)
        .ok_or_else(|| format!("Handoff request not found: {request_id}"))?;
    req.status = if accept {
        HandoffStatus::Accepted
    } else {
        HandoffStatus::Declined
    };
    Ok(())
}

/// Return all handoff requests.
#[tauri::command]
pub fn team_handoff_list() -> Result<Vec<SessionHandoffRequest>, String> {
    let handoffs = get_handoffs();
    let guard = handoffs.lock().map_err(|e| e.to_string())?;
    Ok(guard.clone())
}

// ── Unit tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shared_session(id: &str) -> SharedSession {
        SharedSession {
            id: id.to_string(),
            name: format!("Session {id}"),
            host: "10.0.0.1".to_string(),
            port: 22,
            protocol: "ssh".to_string(),
            owner_id: "owner-1".to_string(),
            read_only_for: vec![],
            tags: vec![],
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_presence(member_id: &str) -> PresenceEntry {
        PresenceEntry {
            member_id: member_id.to_string(),
            display_name: format!("User {member_id}"),
            connected_host: "10.0.0.1".to_string(),
            connected_at: "2026-01-01T00:00:00Z".to_string(),
            session_id: "sess-1".to_string(),
        }
    }

    fn make_handoff(id: &str, from: &str, to: &str) -> SessionHandoffRequest {
        SessionHandoffRequest {
            id: id.to_string(),
            from_member_id: from.to_string(),
            to_member_id: to.to_string(),
            session_id: "sess-1".to_string(),
            message: None,
            status: HandoffStatus::Pending,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // Helpers to work with fresh in-process state: because OnceLock is global,
    // we operate directly on the inner Mutex so tests stay deterministic by
    // clearing state before each logical sub-test.

    fn clear_library() {
        let lib = get_library();
        let mut g = lib.lock().unwrap();
        g.sessions.clear();
        g.last_synced = None;
    }

    fn clear_presence() {
        let p = get_presence();
        let mut g = p.lock().unwrap();
        g.clear();
    }

    fn clear_handoffs() {
        let h = get_handoffs();
        let mut g = h.lock().unwrap();
        g.clear();
    }

    #[test]
    fn test_team_library_publish_and_list() {
        clear_library();

        team_session_publish(make_shared_session("s1")).unwrap();
        team_session_publish(make_shared_session("s2")).unwrap();

        let list = team_session_list().unwrap();
        assert_eq!(list.len(), 2, "expected 2 published sessions");
        assert!(list.iter().any(|s| s.id == "s1"));
        assert!(list.iter().any(|s| s.id == "s2"));
    }

    #[test]
    fn test_team_library_unpublish() {
        clear_library();

        team_session_publish(make_shared_session("del-1")).unwrap();
        assert_eq!(team_session_list().unwrap().len(), 1);

        team_session_unpublish("del-1".to_string()).unwrap();
        assert_eq!(team_session_list().unwrap().len(), 0, "list should be empty after unpublish");
    }

    #[test]
    fn test_presence_update_and_list() {
        clear_presence();

        team_presence_update(make_presence("m1")).unwrap();
        team_presence_update(make_presence("m2")).unwrap();

        let list = team_presence_list().unwrap();
        assert_eq!(list.len(), 2, "expected 2 presence entries");
        assert!(list.iter().any(|e| e.member_id == "m1"));
        assert!(list.iter().any(|e| e.member_id == "m2"));
    }

    #[test]
    fn test_presence_clear() {
        clear_presence();

        team_presence_update(make_presence("clear-me")).unwrap();
        assert_eq!(team_presence_list().unwrap().len(), 1);

        team_presence_clear("clear-me".to_string()).unwrap();
        assert_eq!(
            team_presence_list().unwrap().len(),
            0,
            "presence list should be empty after clear"
        );
    }

    #[test]
    fn test_handoff_request_and_respond() {
        clear_handoffs();

        let req = make_handoff("h1", "alice", "bob");
        team_handoff_request(req).unwrap();

        // Accept the handoff
        team_handoff_respond("h1".to_string(), true).unwrap();

        let list = team_handoff_list().unwrap();
        let h = list.iter().find(|r| r.id == "h1").expect("handoff h1 not found");
        assert_eq!(
            h.status,
            HandoffStatus::Accepted,
            "status should be Accepted after responding with accept=true"
        );
    }

    #[test]
    fn test_handoff_list() {
        clear_handoffs();

        team_handoff_request(make_handoff("hl-1", "u1", "u2")).unwrap();
        team_handoff_request(make_handoff("hl-2", "u2", "u3")).unwrap();

        let list = team_handoff_list().unwrap();
        assert_eq!(list.len(), 2, "expected 2 handoff requests");
        assert!(list.iter().any(|r| r.id == "hl-1"));
        assert!(list.iter().any(|r| r.id == "hl-2"));
    }
}
