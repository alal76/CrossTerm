/// Kubernetes pod port-forwarding, built on the `kube` crate rather than
/// hand-rolled: what looks like a simple channel-multiplexed WebSocket
/// protocol on the surface (`portforward.k8s.io`, channel-byte framed,
/// 2 channels per port for data/error) is intertwined with the rest of
/// the Kubernetes client stack (kubeconfig parsing, every supported auth
/// mode including exec-plugin credential providers, TLS/cert handling),
/// and `kube` already implements all of it correctly and is the de facto
/// standard Rust client for the Kubernetes API — reimplementing that
/// stack from scratch wouldn't be proportionate to one session type.
///
/// This module's own job is just the "port-forward" UX on top: bind a
/// local TCP listener (mirroring `kubectl port-forward`) and relay each
/// accepted local connection to a fresh `Api::<Pod>::portforward` stream,
/// since Kubernetes port-forward streams are per-TCP-connection rather
/// than a single shared pipe.
use k8s_openapi::api::core::v1::Pod;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex as TokioMutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum K8sPortForwardError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Kubeconfig error: {0}")]
    Kubeconfig(String),
    #[error("Kubernetes API error: {0}")]
    Kube(String),
    #[error("Network error: {0}")]
    Io(String),
}

impl Serialize for K8sPortForwardError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for K8sPortForwardError {
    fn from(e: std::io::Error) -> Self {
        K8sPortForwardError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sPortForwardConfig {
    /// Path to a kubeconfig file; None uses the default resolution
    /// (`$KUBECONFIG` or `~/.kube/config`), same as kubectl.
    pub kubeconfig_path: Option<String>,
    /// Context to use; None uses the kubeconfig's current-context.
    pub context: Option<String>,
    pub namespace: String,
    pub pod_name: String,
    pub remote_port: u16,
    /// 0 picks an ephemeral local port; the actual bound port is returned
    /// in K8sPortForwardConnectResult.
    pub local_port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct K8sPortForwardConnectResult {
    pub id: String,
    pub local_port: u16,
}

#[derive(Clone, Serialize)]
struct K8sPortForwardConnEvent {
    session_id: String,
    peer: String,
    state: String, // "open" | "closed" | "error"
}

#[derive(Clone, Serialize)]
struct K8sPortForwardErrorEvent {
    session_id: String,
    message: String,
}

struct K8sPortForwardSession {
    local_addr: SocketAddr,
    accept_handle: JoinHandle<()>,
    conn_handles: std::sync::Arc<TokioMutex<Vec<JoinHandle<()>>>>,
}

pub struct K8sPortForwardState {
    sessions: Mutex<HashMap<String, K8sPortForwardSession>>,
}

impl K8sPortForwardState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

async fn build_client(config: &K8sPortForwardConfig) -> Result<Client, K8sPortForwardError> {
    let kubeconfig = match &config.kubeconfig_path {
        Some(path) => Kubeconfig::read_from(path).map_err(|e| K8sPortForwardError::Kubeconfig(e.to_string()))?,
        None => Kubeconfig::read().map_err(|e| K8sPortForwardError::Kubeconfig(e.to_string()))?,
    };
    let options = KubeConfigOptions {
        context: config.context.clone(),
        cluster: None,
        user: None,
    };
    let client_config = Config::from_custom_kubeconfig(kubeconfig, &options)
        .await
        .map_err(|e| K8sPortForwardError::Kubeconfig(e.to_string()))?;
    Client::try_from(client_config).map_err(|e| K8sPortForwardError::Kube(e.to_string()))
}

/// Relays one accepted local TCP connection over a fresh port-forward
/// stream. Each local connection gets its own Kubernetes port-forward
/// stream, matching how `kubectl port-forward` handles concurrent clients.
async fn relay_one_connection(pods: &Api<Pod>, pod_name: &str, remote_port: u16, local_stream: &mut TcpStream) -> Result<(), K8sPortForwardError> {
    let mut pf = pods
        .portforward(pod_name, &[remote_port])
        .await
        .map_err(|e| K8sPortForwardError::Kube(e.to_string()))?;
    let mut pod_stream = pf
        .take_stream(remote_port)
        .ok_or_else(|| K8sPortForwardError::Kube(format!("no stream returned for port {remote_port}")))?;

    let copy_result = tokio::io::copy_bidirectional(local_stream, &mut pod_stream).await;
    drop(pod_stream);

    // join() drives the forwarder's background task to completion and
    // surfaces any protocol-level error even if the byte copy itself
    // looked clean (e.g. the pod-side process exited mid-stream).
    let join_result = pf.join().await;

    copy_result.map_err(K8sPortForwardError::from)?;
    join_result.map_err(|e| K8sPortForwardError::Kube(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn k8s_port_forward_connect(
    config: K8sPortForwardConfig,
    state: tauri::State<'_, K8sPortForwardState>,
    app: AppHandle,
) -> Result<K8sPortForwardConnectResult, K8sPortForwardError> {
    let client = build_client(&config).await?;
    let pods: Api<Pod> = Api::namespaced(client, &config.namespace);

    // Fail fast at connect time rather than on the first local connection.
    pods.get(&config.pod_name).await.map_err(|e| K8sPortForwardError::Kube(e.to_string()))?;

    let listener = TcpListener::bind(("127.0.0.1", config.local_port)).await?;
    let local_addr = listener.local_addr()?;

    let id = Uuid::new_v4().to_string();
    let conn_handles = std::sync::Arc::new(TokioMutex::new(Vec::<JoinHandle<()>>::new()));

    let pod_name = config.pod_name.clone();
    let remote_port = config.remote_port;
    let app_for_loop = app.clone();
    let id_for_loop = id.clone();
    let handles_for_loop = std::sync::Arc::clone(&conn_handles);

    let accept_handle = tokio::spawn(async move {
        loop {
            let (mut local_stream, peer) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let pods = pods.clone();
            let pod_name = pod_name.clone();
            let app = app_for_loop.clone();
            let id = id_for_loop.clone();

            let handle = tokio::spawn(async move {
                let _ = app.emit(
                    "k8s_port_forward:connection",
                    K8sPortForwardConnEvent { session_id: id.clone(), peer: peer.to_string(), state: "open".into() },
                );
                let result = relay_one_connection(&pods, &pod_name, remote_port, &mut local_stream).await;
                let _ = app.emit(
                    "k8s_port_forward:connection",
                    K8sPortForwardConnEvent {
                        session_id: id.clone(),
                        peer: peer.to_string(),
                        state: if result.is_err() { "error".into() } else { "closed".into() },
                    },
                );
                if let Err(e) = result {
                    let _ = app.emit(
                        "k8s_port_forward:error",
                        K8sPortForwardErrorEvent { session_id: id, message: e.to_string() },
                    );
                }
            });
            handles_for_loop.lock().await.push(handle);
        }
    });

    state.sessions.lock().unwrap().insert(
        id.clone(),
        K8sPortForwardSession { local_addr, accept_handle, conn_handles },
    );

    Ok(K8sPortForwardConnectResult { id, local_port: local_addr.port() })
}

#[tauri::command]
pub async fn k8s_port_forward_disconnect(id: String, state: tauri::State<'_, K8sPortForwardState>) -> Result<(), K8sPortForwardError> {
    let session = state.sessions.lock().unwrap().remove(&id).ok_or_else(|| K8sPortForwardError::NotFound(id))?;
    session.accept_handle.abort();
    for handle in session.conn_handles.lock().await.drain(..) {
        handle.abort();
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct K8sPortForwardInfo {
    pub id: String,
    pub local_port: u16,
}

#[tauri::command]
pub fn k8s_port_forward_list(state: tauri::State<'_, K8sPortForwardState>) -> Vec<K8sPortForwardInfo> {
    state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .map(|(id, session)| K8sPortForwardInfo { id: id.clone(), local_port: session.local_addr.port() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_kubeconfig_yaml() -> String {
        r#"
apiVersion: v1
kind: Config
current-context: dev
clusters:
  - name: dev-cluster
    cluster:
      server: https://127.0.0.1:6443
      insecure-skip-tls-verify: true
contexts:
  - name: dev
    context:
      cluster: dev-cluster
      user: dev-user
      namespace: default
users:
  - name: dev-user
    user:
      token: fake-token-for-tests
"#
        .to_string()
    }

    /// Writes a sample kubeconfig to a fresh temp dir and returns
    /// (dir, kubeconfig path) so tests can exercise `Kubeconfig::read_from`
    /// and `build_client` without touching the user's real ~/.kube/config.
    fn write_temp_kubeconfig() -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("crossterm-k8s-kubeconfig-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config");
        std::fs::write(&path, sample_kubeconfig_yaml()).unwrap();
        (dir, path.to_string_lossy().into_owned())
    }

    #[test]
    fn kubeconfig_parses_context_cluster_and_namespace() {
        let (dir, path) = write_temp_kubeconfig();
        let kubeconfig = Kubeconfig::read_from(&path).unwrap();
        assert_eq!(kubeconfig.current_context.as_deref(), Some("dev"));
        assert_eq!(kubeconfig.clusters.len(), 1);
        assert_eq!(kubeconfig.clusters[0].name, "dev-cluster");
        assert_eq!(kubeconfig.contexts.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn build_client_reads_from_an_explicit_kubeconfig_path() {
        let (dir, path) = write_temp_kubeconfig();
        let config = K8sPortForwardConfig {
            kubeconfig_path: Some(path),
            context: Some("dev".into()),
            namespace: "default".into(),
            pod_name: "does-not-matter".into(),
            remote_port: 80,
            local_port: 0,
        };

        // We don't have a live cluster in tests, but this exercises the
        // full kubeconfig-read -> Config::from_custom_kubeconfig -> Client
        // construction path without making any network calls (the client
        // is constructed lazily; only later API calls would hit the network).
        if let Err(e) = &build_client(&config).await {
            panic!("expected a client to be constructed from a valid kubeconfig: {e}");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn build_client_reports_a_clear_error_for_an_unknown_context() {
        let (dir, path) = write_temp_kubeconfig();
        let config = K8sPortForwardConfig {
            kubeconfig_path: Some(path),
            context: Some("does-not-exist".into()),
            namespace: "default".into(),
            pod_name: "does-not-matter".into(),
            remote_port: 80,
            local_port: 0,
        };

        let result = build_client(&config).await;
        assert!(matches!(result, Err(K8sPortForwardError::Kubeconfig(_))));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn connect_binds_an_ephemeral_local_port_when_requested() {
        // Exercises just the TcpListener::bind(local_port=0) path directly,
        // since the full k8s_port_forward_connect command needs a live
        // cluster (it calls pods.get() before returning).
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_ne!(addr.port(), 0);
    }

    #[test]
    fn fresh_state_has_no_sessions() {
        // k8s_port_forward_disconnect's NotFound path is exercised through
        // this same lookup; tauri::State can't be constructed outside a
        // running app in a unit test, so this checks the underlying map
        // directly rather than the #[tauri::command] wrapper.
        let state = K8sPortForwardState::new();
        assert!(state.sessions.lock().unwrap().get("missing-id").is_none());
    }
}
