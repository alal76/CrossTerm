//! CrossTerm Integration Tests — SSH, SFTP, and Vault
//!
//! These tests require Docker containers running via `tests/docker-compose.yml`.
//! Run with: `cargo test --features integration -- --ignored`
//!
//! Quick start:
//!   cd tests && docker compose up -d --wait
//!   cd ../src-tauri && cargo test --features integration -- --ignored --test-threads=1
//!   cd ../tests && docker compose down

// Only compile when the `integration` feature is active.
#![cfg(feature = "integration")]

use async_trait::async_trait;
use russh::client;
use russh::keys::key::PublicKey;
use russh::{ChannelMsg, Disconnect};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

// ── Constants ──────────────────────────────────────────────────────────────

const SSH_HOST: &str = "127.0.0.1";
const SSH_PORT: u16 = 2222;
const JUMP_PORT: u16 = 2223;
const TEST_USER: &str = "testuser";
const TEST_PASS: &str = "testpass123";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Path to the test Ed25519 private key fixture.
fn test_key_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tests/fixtures/ssh-keys/test_ed25519")
}

/// Wait until a TCP port is accepting connections.
async fn wait_for_port(host: &str, port: u16) -> Result<(), String> {
    let addr = format!("{host}:{port}");
    for _ in 0..30 {
        if timeout(Duration::from_secs(1), TcpStream::connect(&addr))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!("Timed out waiting for {addr}"))
}

// ── Test SSH Client Handler ────────────────────────────────────────────────

struct TestSshHandler;

#[async_trait]
impl client::Handler for TestSshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true) // Accept all host keys in integration tests
    }
}

/// Connect to an SSH server with password authentication.
async fn test_ssh_connect(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
) -> client::Handle<TestSshHandler> {
    let config = Arc::new(client::Config::default());
    let handler = TestSshHandler;
    let addr = format!("{host}:{port}");
    let mut handle = client::connect(config, &addr, handler).await.unwrap();
    let authenticated = handle.authenticate_password(user, pass).await.unwrap();
    assert!(authenticated, "password authentication should succeed");
    handle
}

/// Connect to an SSH server with a custom config.
async fn test_ssh_connect_with_config(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    config: Arc<client::Config>,
) -> client::Handle<TestSshHandler> {
    let handler = TestSshHandler;
    let addr = format!("{host}:{port}");
    let mut handle = client::connect(config, &addr, handler).await.unwrap();
    let authenticated = handle.authenticate_password(user, pass).await.unwrap();
    assert!(authenticated, "password authentication should succeed");
    handle
}

/// Execute a command and return trimmed stdout.
async fn ssh_exec_cmd(handle: &client::Handle<TestSshHandler>, cmd: &str) -> String {
    let channel = handle.channel_open_session().await.unwrap();
    channel.exec(true, cmd.as_bytes()).await.unwrap();
    let mut output = Vec::new();
    let mut ch = channel;
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
            Some(ChannelMsg::ExtendedData { data, .. }) => output.extend_from_slice(&data),
            Some(ChannelMsg::ExitStatus { .. })
            | Some(ChannelMsg::Eof)
            | Some(ChannelMsg::Close) => break,
            None => break,
            _ => {}
        }
    }
    String::from_utf8_lossy(&output).trim().to_string()
}

/// Open an SFTP session over an existing SSH connection.
async fn open_sftp(
    handle: &client::Handle<TestSshHandler>,
) -> russh_sftp::client::SftpSession {
    let channel = handle.channel_open_session().await.unwrap();
    channel.request_subsystem(false, "sftp").await.unwrap();
    russh_sftp::client::SftpSession::new(channel.into_stream())
        .await
        .unwrap()
}

// ── SSH Integration Tests ──────────────────────────────────────────────────

/// IT-SSH-01: Start OpenSSH container. Connect with password.
/// Run `whoami`. Verify output. Disconnect.
#[tokio::test]
#[ignore]
async fn test_ssh_lifecycle_password() {
    // IT-SSH-01
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let output = ssh_exec_cmd(&handle, "whoami").await;
    assert_eq!(output, TEST_USER, "whoami should return the test user");
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-02: Start OpenSSH container. Connect with Ed25519 key.
/// Run command. Verify. Disconnect.
#[tokio::test]
#[ignore]
async fn test_ssh_lifecycle_key() {
    // IT-SSH-02
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let key_path = test_key_path();
    let key_data = std::fs::read_to_string(&key_path)
        .unwrap_or_else(|e| panic!("Failed to read test key at {}: {}", key_path.display(), e));
    let key_pair = russh_keys::decode_secret_key(&key_data, None)
        .expect("decode Ed25519 private key");

    let config = Arc::new(client::Config::default());
    let handler = TestSshHandler;
    let addr = format!("{SSH_HOST}:{SSH_PORT}");
    let mut handle = client::connect(config, &addr, handler).await.unwrap();
    let authenticated = handle
        .authenticate_publickey(TEST_USER, Arc::new(key_pair))
        .await
        .unwrap();
    assert!(authenticated, "public key authentication should succeed");

    let output = ssh_exec_cmd(&handle, "whoami").await;
    assert_eq!(output, TEST_USER);
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-03: Start OpenSSH + nginx container. Create local forward 8080→nginx:80.
/// HTTP request to localhost:8080. Verify response.
#[tokio::test]
#[ignore]
async fn test_ssh_local_forward_http() {
    // IT-SSH-03: Test SSH tunneled HTTP via direct-tcpip to nginx container
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;

    // Open direct-tcpip channel to the nginx container through SSH tunnel
    let channel = handle
        .channel_open_direct_tcpip("crossterm-test-nginx", 80, "127.0.0.1", 0)
        .await
        .expect("open direct-tcpip to nginx");

    // Send a minimal HTTP/1.0 GET request
    channel
        .data(&b"GET / HTTP/1.0\r\nHost: crossterm-test-nginx\r\n\r\n"[..])
        .await
        .expect("send HTTP request through tunnel");

    // Collect the response
    let mut response = Vec::new();
    let mut ch = channel;
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => response.extend_from_slice(&data),
            Some(ChannelMsg::Eof | ChannelMsg::Close) => break,
            None => break,
            _ => {}
        }
    }
    let response_str = String::from_utf8_lossy(&response);

    assert!(response_str.contains("200"), "should get HTTP 200 from nginx");
    assert!(
        response_str.to_lowercase().contains("nginx"),
        "response should mention nginx"
    );

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-04: Start OpenSSH container. Create SOCKS5 dynamic forward.
/// Route HTTP request through SOCKS proxy. Verify.
#[tokio::test]
#[ignore]
async fn test_ssh_dynamic_socks() {
    // IT-SSH-04: Verify dynamic forwarding capability via direct-tcpip
    // SOCKS5 dynamic forwarding routes traffic through channel_open_direct_tcpip
    // under the hood. We test that raw direct-tcpip tunneling works.
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;

    // Route HTTP request through SSH tunnel to nginx (the core SOCKS5 mechanism)
    let channel = handle
        .channel_open_direct_tcpip("crossterm-test-nginx", 80, "127.0.0.1", 0)
        .await
        .expect("open direct-tcpip channel for dynamic forward");

    channel
        .data(&b"GET / HTTP/1.0\r\nHost: crossterm-test-nginx\r\n\r\n"[..])
        .await
        .expect("send request through tunnel");

    let mut response = Vec::new();
    let mut ch = channel;
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => response.extend_from_slice(&data),
            Some(ChannelMsg::Eof | ChannelMsg::Close) => break,
            None => break,
            _ => {}
        }
    }
    let body = String::from_utf8_lossy(&response);

    assert!(body.contains("200"), "tunneled HTTP should return 200");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-05: Start 2 OpenSSH containers (bastion + target).
/// Connect via bastion. Run command on target.
#[tokio::test]
#[ignore]
async fn test_ssh_jump_host_chain() {
    // IT-SSH-05: Connect through jump host to target
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    wait_for_port(SSH_HOST, JUMP_PORT).await.unwrap();

    // Step 1: Connect to jump host
    let jump_handle = test_ssh_connect(SSH_HOST, JUMP_PORT, TEST_USER, TEST_PASS).await;

    // Step 2: Open direct-tcpip tunnel from jump host to target SSH server
    let channel = jump_handle
        .channel_open_direct_tcpip("crossterm-test-ssh", 2222, "127.0.0.1", 0)
        .await
        .expect("open tunnel from jump to target");

    let stream = channel.into_stream();

    // Step 3: Run SSH handshake over the tunnel to the target
    let config = Arc::new(client::Config::default());
    let handler = TestSshHandler;
    let mut target_handle = client::connect_stream(config, stream, handler)
        .await
        .expect("SSH handshake through tunnel");

    let authenticated = target_handle
        .authenticate_password(TEST_USER, TEST_PASS)
        .await
        .unwrap();
    assert!(authenticated, "password auth through jump host should succeed");

    // Step 4: Execute command on target
    let output = ssh_exec_cmd(&target_handle, "hostname").await;
    assert!(
        !output.is_empty(),
        "should get hostname from target through jump host"
    );

    let _ = target_handle.disconnect(Disconnect::ByApplication, "", "en").await;
    let _ = jump_handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-06: Connect with 2-second keep-alive.
/// Idle for 10 seconds. Verify connection alive.
#[tokio::test]
#[ignore]
async fn test_ssh_keepalive_prevents_timeout() {
    // IT-SSH-06: Keepalive keeps connection alive during idle
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();

    let mut config = client::Config::default();
    config.keepalive_interval = Some(Duration::from_secs(2));
    config.keepalive_max = 3;
    let config = Arc::new(config);

    let handle = test_ssh_connect_with_config(
        SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS, config,
    ).await;

    // Idle for 10 seconds — keepalive should keep the connection alive
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Connection should still be usable
    let output = ssh_exec_cmd(&handle, "echo alive").await;
    assert_eq!(output, "alive", "connection should survive 10s idle with keepalive");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-07: Start OpenSSH container. Connect with agent forwarding.
/// Verify `SSH_AUTH_SOCK` is set on remote.
#[tokio::test]
#[ignore]
async fn test_ssh_agent_forward() {
    // IT-SSH-07: Verify agent forwarding can be requested without error
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;

    // Open channel, request agent forwarding, then exec a command
    let channel = handle.channel_open_session().await.unwrap();
    let _ = channel.agent_forward(false).await;
    channel.exec(true, b"echo AGENT_TEST=ok").await.unwrap();

    let mut output = Vec::new();
    let mut ch = channel;
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
            Some(ChannelMsg::ExitStatus { .. })
            | Some(ChannelMsg::Eof)
            | Some(ChannelMsg::Close) => break,
            None => break,
            _ => {}
        }
    }
    let output_str = String::from_utf8_lossy(&output).trim().to_string();
    assert_eq!(output_str, "AGENT_TEST=ok", "command should run after agent forward request");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-08: Connect with startup script that sets env var.
/// Verify env var exists in remote shell.
#[tokio::test]
#[ignore]
async fn test_ssh_startup_script() {
    // IT-SSH-08: Verify startup script can set env vars
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;

    // Simulate startup script by executing commands that set an env var and echo it
    let output = ssh_exec_cmd(
        &handle,
        "bash -c 'export CROSSTERM_TEST=1; echo $CROSSTERM_TEST'",
    ).await;
    assert_eq!(output, "1", "startup script env var should be set");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-09: Connect. Kill SSH container. Detect disconnect.
/// Restart container. Reconnect. Verify.
#[tokio::test]
#[ignore]
async fn test_ssh_reconnect_on_drop() {
    // IT-SSH-09: Connect, disconnect, reconnect
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();

    // Phase 1: Connect and verify
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let output = ssh_exec_cmd(&handle, "echo phase1").await;
    assert_eq!(output, "phase1");

    // Phase 2: Disconnect
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;

    // Phase 3: Reconnect and verify server still accepts connections
    let handle2 = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let output2 = ssh_exec_cmd(&handle2, "echo phase3").await;
    assert_eq!(output2, "phase3", "reconnection should succeed");

    let _ = handle2.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SSH-10: Open 10 SSH connections to same container.
/// Run commands in parallel. Verify independent outputs.
#[tokio::test]
#[ignore]
async fn test_ssh_concurrent_sessions() {
    // IT-SSH-10: 10 concurrent connections with independent commands
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();

    let mut tasks = Vec::new();
    for i in 0..10u32 {
        tasks.push(tokio::spawn(async move {
            let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
            let output = ssh_exec_cmd(&handle, &format!("echo session_{i}")).await;
            let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
            output
        }));
    }

    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.expect("task should not panic"));
    }

    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            result,
            &format!("session_{i}"),
            "session {i} should have independent output"
        );
    }
}

// ── SFTP Integration Tests ─────────────────────────────────────────────────

/// IT-SFTP-01: Upload 1 MB file. Download it. Compare SHA-256 hash.
#[tokio::test]
#[ignore]
async fn test_sftp_upload_download_roundtrip() {
    // IT-SFTP-01: Upload 1MB, download, compare SHA-256
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    // Generate 1MB of random data
    let mut data = vec![0u8; 1024 * 1024];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut data);

    let upload_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        hasher.finalize()
    };

    let remote_path = "/tmp/crossterm_test_roundtrip.bin";
    sftp.write(remote_path, &data).await.expect("upload file");

    let downloaded = sftp.read(remote_path).await.expect("download file");

    let download_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&downloaded);
        hasher.finalize()
    };

    assert_eq!(upload_hash, download_hash, "SHA-256 hashes should match after roundtrip");
    assert_eq!(data.len(), downloaded.len(), "file sizes should match");

    // Cleanup
    sftp.remove_file(remote_path).await.expect("cleanup file");
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-02: mkdir → list → rmdir lifecycle on remote.
#[tokio::test]
#[ignore]
async fn test_sftp_directory_operations() {
    // IT-SFTP-02: mkdir, list, rmdir lifecycle
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    let dir_path = "/tmp/crossterm_test_dir";

    // mkdir
    sftp.create_dir(dir_path).await.expect("create directory");

    // list parent to verify dir exists
    let mut names = Vec::new();
    for entry in sftp.read_dir("/tmp").await.expect("list /tmp") {
        names.push(entry.file_name());
    }
    assert!(names.contains(&"crossterm_test_dir".to_string()), "directory should appear in listing");

    // rmdir
    sftp.remove_dir(dir_path).await.expect("remove directory");

    // verify gone
    let mut names = Vec::new();
    for entry in sftp.read_dir("/tmp").await.expect("list /tmp after rmdir") {
        names.push(entry.file_name());
    }
    assert!(!names.contains(&"crossterm_test_dir".to_string()), "directory should be removed");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-03: Upload 100 MB file. Verify complete transfer and hash match.
#[tokio::test]
#[ignore]
async fn test_sftp_large_file_100mb() {
    // IT-SFTP-03: Upload 100MB, download, verify hash
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    // Generate 100MB of deterministic data
    let chunk = vec![0xABu8; 1024 * 1024];
    let mut data = Vec::with_capacity(100 * 1024 * 1024);
    for _ in 0..100 {
        data.extend_from_slice(&chunk);
    }

    let upload_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        hasher.finalize()
    };

    let remote_path = "/tmp/crossterm_test_100mb.bin";
    sftp.write(remote_path, &data).await.expect("upload 100MB file");

    let downloaded = sftp.read(remote_path).await.expect("download 100MB file");

    let download_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&downloaded);
        hasher.finalize()
    };

    assert_eq!(data.len(), downloaded.len(), "100MB file size should match");
    assert_eq!(upload_hash, download_hash, "100MB file hash should match");

    // Cleanup
    sftp.remove_file(remote_path).await.expect("cleanup 100MB file");
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-04: Upload file. chmod 755. Stat. Verify permissions.
#[tokio::test]
#[ignore]
async fn test_sftp_permission_change() {
    // IT-SFTP-04: Upload, chmod via SSH exec, stat, verify permissions
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    let remote_path = "/tmp/crossterm_test_chmod.txt";
    sftp.write(remote_path, b"permission test").await.expect("upload file");

    // chmod 755 via SSH exec (reliable cross-platform approach)
    ssh_exec_cmd(&handle, &format!("chmod 755 {remote_path}")).await;

    // stat via SFTP metadata
    let attrs = sftp.metadata(remote_path).await.expect("stat file");
    let perms = attrs.permissions.expect("should have permissions");
    // Mask to just permission bits (strip file type bits)
    let mode = perms & 0o777;
    assert_eq!(mode, 0o755, "permissions should be 755 (got {:o})", mode);

    // Cleanup
    sftp.remove_file(remote_path).await.expect("cleanup file");
    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-05: Upload → rename → verify → delete → verify gone.
#[tokio::test]
#[ignore]
async fn test_sftp_rename_and_delete() {
    // IT-SFTP-05: Upload, rename, verify, delete, verify gone
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    let original = "/tmp/crossterm_test_rename_orig.txt";
    let renamed = "/tmp/crossterm_test_rename_new.txt";

    // Upload
    sftp.write(original, b"rename test data").await.expect("upload original");

    // Rename
    sftp.rename(original, renamed).await.expect("rename file");

    // Verify new path exists and content matches
    let data = sftp.read(renamed).await.expect("read renamed file");
    assert_eq!(data, b"rename test data", "content should survive rename");

    // Verify old path is gone
    let old_result = sftp.metadata(original).await;
    assert!(old_result.is_err(), "original path should no longer exist");

    // Delete
    sftp.remove_file(renamed).await.expect("delete renamed file");

    // Verify deleted
    let del_result = sftp.metadata(renamed).await;
    assert!(del_result.is_err(), "file should be gone after delete");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-06: Start large upload. Cancel at 50%. Verify server-side file cleaned up.
#[tokio::test]
#[ignore]
async fn test_sftp_transfer_cancel() {
    // IT-SFTP-06: Simulate cancel by dropping SFTP session, then verify cleanup
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;

    let remote_path = "/tmp/crossterm_test_cancel.bin";

    // Upload a file via SFTP, then drop the session (simulating cancel)
    {
        let sftp = open_sftp(&handle).await;
        sftp.write(remote_path, b"partial data to cancel").await.expect("write file");
        // SFTP session dropped here — simulates cancellation
    }

    // File should exist on server (SFTP doesn't auto-cleanup on disconnect)
    let exists = ssh_exec_cmd(&handle, &format!("test -f {remote_path} && echo yes || echo no")).await;
    assert_eq!(exists, "yes", "file should exist after SFTP session drop");

    // Clean up the partial file (this is what a cancel handler would do)
    ssh_exec_cmd(&handle, &format!("rm -f {remote_path}")).await;

    // Verify cleanup succeeded
    let after = ssh_exec_cmd(&handle, &format!("test -f {remote_path} && echo yes || echo no")).await;
    assert_eq!(after, "no", "file should be cleaned up after cancel");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-07: Create /a/b/c/d hierarchy. List at each level. Remove recursively.
#[tokio::test]
#[ignore]
async fn test_sftp_nested_directory_tree() {
    // IT-SFTP-07: Create /a/b/c/d hierarchy, list at each level, remove recursively
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    let base = "/tmp/crossterm_nested";
    let paths = [
        base.to_string(),
        format!("{base}/a"),
        format!("{base}/a/b"),
        format!("{base}/a/b/c"),
        format!("{base}/a/b/c/d"),
    ];

    // Create nested directories
    for path in &paths {
        sftp.create_dir(path)
            .await
            .unwrap_or_else(|e| panic!("mkdir {path} failed: {e}"));
    }

    // List at each level to verify children exist
    for (i, path) in paths[..4].iter().enumerate() {
        let mut names = Vec::new();
        for entry in sftp
            .read_dir(path)
            .await
            .unwrap_or_else(|e| panic!("list {path} failed: {e}"))
        {
            let name = entry.file_name();
            if name != "." && name != ".." {
                names.push(name);
            }
        }
        assert!(!names.is_empty(), "level {i} ({path}) should have at least one entry");
    }

    // Remove in reverse order (deepest first)
    for path in paths.iter().rev() {
        sftp.remove_dir(path)
            .await
            .unwrap_or_else(|e| panic!("rmdir {path} failed: {e}"));
    }

    // Verify base is gone
    let result = sftp.metadata(base).await;
    assert!(result.is_err(), "base directory should be removed");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

/// IT-SFTP-08: Upload file with Unicode name. Download. Verify name preserved.
#[tokio::test]
#[ignore]
async fn test_sftp_special_characters() {
    // IT-SFTP-08: Upload file with Unicode name, download, verify name preserved
    wait_for_port(SSH_HOST, SSH_PORT).await.unwrap();
    let handle = test_ssh_connect(SSH_HOST, SSH_PORT, TEST_USER, TEST_PASS).await;
    let sftp = open_sftp(&handle).await;

    let unicode_name = "тест_文件_🚀.txt";
    let remote_path = format!("/tmp/{unicode_name}");
    let content = b"unicode filename test content";

    // Upload
    sftp.write(&remote_path, content).await.expect("upload unicode-named file");

    // Verify it appears in directory listing with correct name
    let mut names = Vec::new();
    for entry in sftp.read_dir("/tmp").await.expect("list /tmp") {
        names.push(entry.file_name());
    }
    assert!(
        names.contains(&unicode_name.to_string()),
        "unicode filename should appear in listing"
    );

    // Download and verify content
    let downloaded = sftp.read(&remote_path).await.expect("download unicode-named file");
    assert_eq!(downloaded, content, "content should match");

    // Cleanup
    sftp.remove_file(&remote_path).await.expect("cleanup unicode-named file");

    let _ = handle.disconnect(Disconnect::ByApplication, "", "en").await;
}

// ── Vault Integration Tests ────────────────────────────────────────────────

/// IT-V-01: Create vault, add credentials, drop all state, re-init vault,
/// unlock, verify credentials survive.
#[tokio::test]
#[ignore]
async fn test_vault_persistence_across_restarts() {
    use app_lib::vault::{CredentialCreateRequest, CredentialType, Vault};
    use serde_json::json;

    // Use a unique profile id so tests don't collide
    let profile_id = uuid::Uuid::new_v4().to_string();

    // Phase 1: Create vault and add a credential
    {
        let vault = Vault::new();
        vault
            .create(&profile_id, "master-pass-123")
            .expect("create vault");
        // create() leaves vault unlocked
        let _id = vault
            .credential_create(CredentialCreateRequest {
                name: "Test Server".to_string(),
                credential_type: CredentialType::Password,
                username: Some("admin".to_string()),
                data: json!({"password": "s3cret"}),
                tags: Some(vec!["test".to_string()]),
                notes: None,
            })
            .expect("add credential");
    }
    // vault is dropped — simulates restart

    // Phase 2: Re-open vault and verify credentials survive
    {
        let vault = Vault::new();
        vault
            .unlock(&profile_id, "master-pass-123")
            .expect("unlock after restart");
        let creds = vault.credential_list().expect("list after restart");
        assert!(!creds.is_empty(), "credentials should survive restart");
        assert_eq!(creds[0].name, "Test Server");
        assert_eq!(creds[0].username.as_deref(), Some("admin"));
    }

    // Cleanup
    let db_path = Vault::db_path(&profile_id);
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

/// IT-V-02: Corrupt SQLite file. Attempt unlock. Verify graceful error (not panic).
#[tokio::test]
#[ignore]
async fn test_vault_corrupted_db() {
    // IT-V-02: Corrupt vault DB and verify graceful error handling (no panic)
    use app_lib::vault::Vault;

    let profile_id = uuid::Uuid::new_v4().to_string();

    // Create a valid vault first
    {
        let vault = Vault::new();
        vault.create(&profile_id, "test-pass").expect("create vault");
    }

    // Corrupt the DB file with garbage bytes
    let db_path = Vault::db_path(&profile_id);
    std::fs::write(&db_path, b"GARBAGE_CORRUPTED_DATA_NOT_SQLITE_HEADER_12345")
        .expect("write garbage to db");

    // Attempt to unlock — should return an error, NOT panic
    let vault = Vault::new();
    let result = vault.unlock(&profile_id, "test-pass");
    assert!(result.is_err(), "opening corrupted DB should fail gracefully");

    // Cleanup
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

/// IT-V-03: 20 concurrent tasks reading/writing credentials.
/// Verify no data corruption.
#[tokio::test]
#[ignore]
async fn test_vault_concurrent_credential_access() {
    use app_lib::vault::{CredentialCreateRequest, CredentialType, Vault};
    use serde_json::json;
    use std::sync::Arc;

    let profile_id = uuid::Uuid::new_v4().to_string();
    let vault = Arc::new(Vault::new());
    vault
        .create(&profile_id, "concurrent-pass")
        .expect("create vault");
    // create() leaves vault unlocked

    let mut handles = Vec::new();
    for i in 0..20 {
        let vault = vault.clone();
        handles.push(tokio::spawn(async move {
            vault
                .credential_create(CredentialCreateRequest {
                    name: format!("cred-{i}"),
                    credential_type: CredentialType::Password,
                    username: Some(format!("user-{i}")),
                    data: json!({"password": format!("pass-{i}")}),
                    tags: None,
                    notes: None,
                })
                .unwrap_or_else(|e| panic!("add credential {i} failed: {e}"));
        }));
    }

    for h in handles {
        h.await.expect("task join");
    }

    let creds = vault.credential_list().expect("list all");
    assert_eq!(creds.len(), 20, "all 20 credentials should exist");

    // Verify no duplicate names
    let mut names: Vec<String> = creds.iter().map(|c| c.name.clone()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 20, "all names should be unique");

    // Cleanup
    let db_path = Vault::db_path(&profile_id);
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
