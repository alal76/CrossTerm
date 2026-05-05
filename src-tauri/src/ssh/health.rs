use super::*;
use tauri::AppHandle;

/// Returns a `Vec<SessionHealth>` snapshot for all currently active connections.
///
/// Health is derived from `SshConnectionInfo::latency_ms` (as a rough proxy for
/// last observed activity).  Because the SSH connection struct does not yet carry
/// an explicit `last_activity` timestamp or `missed_keepalives` counter, we use
/// `latency_ms` to illustrate the shape and return safe placeholder values for
/// the fields that are not yet tracked.  Future work can replace these with real
/// per-connection activity timestamps.
#[tauri::command]
pub async fn ssh_get_connection_health(
    state: tauri::State<'_, SshState>,
) -> Result<Vec<SessionHealth>, SshError> {
    let connections = state.connections.read().await;
    let mut results = Vec::new();

    for conn_arc in connections.values() {
        let conn = conn_arc.lock().await;
        // Placeholder: no real last-activity tracking yet, so last_seen_secs = 0.
        // The status and missed_keepalives fields will be meaningful once
        // per-connection activity timestamps are added.
        let health = SessionHealth {
            connection_id: conn.info.connection_id.clone(),
            status: SessionHealthStatus::Ok,
            latency_ms: conn.info.latency_ms,
            last_seen_secs: 0,
            missed_keepalives: 0,
        };
        results.push(health);
    }

    Ok(results)
}

/// Spawns a background task that emits `session_health` Tauri events every 15 seconds
/// for all active SSH connections.
///
/// Health thresholds (using `last_seen_secs` once activity tracking is in place):
/// - `Ok`       – last activity ≤ 30 s ago
/// - `Degraded` – last activity > 30 s and ≤ 60 s ago
/// - `Dropped`  – last activity > 60 s ago
///
/// For now, `last_seen_secs` is a placeholder (0) because per-connection activity
/// timestamps are not yet stored on `SshConnection`.  The task still emits health
/// events so the frontend wiring can be validated end-to-end.
#[tauri::command]
pub async fn ssh_start_health_monitor(
    app_handle: AppHandle,
    state: tauri::State<'_, SshState>,
) -> Result<(), SshError> {
    let connections_ref = state.connections.clone();
    let app = app_handle.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;

            let connections = connections_ref.read().await;
            for conn_arc in connections.values() {
                let conn = conn_arc.lock().await;

                // Placeholder activity tracking: once SshConnection gains a
                // `last_activity: Instant` field, replace `elapsed_secs` below
                // with `conn.last_activity.elapsed().as_secs()`.
                let elapsed_secs: u64 = 0;

                let status = if elapsed_secs > 60 {
                    SessionHealthStatus::Dropped
                } else if elapsed_secs > 30 {
                    SessionHealthStatus::Degraded
                } else {
                    SessionHealthStatus::Ok
                };

                let health = SessionHealth {
                    connection_id: conn.info.connection_id.clone(),
                    status,
                    latency_ms: conn.info.latency_ms,
                    last_seen_secs: elapsed_secs,
                    missed_keepalives: 0,
                };

                let _ = app.emit("session_health", &health);
            }
        }
    });

    Ok(())
}
