---
title: "Plugin API Developer Guide"
slug: plugin-api-guide
category: Developer
keywords: [plugin, api, wasm, extension, developer]
schema_version: 1
---

# Plugin API Developer Guide

> **Note**: The Plugin API is planned for CrossTerm Phase 3. This guide documents the planned architecture and API surface. Implementation is in progress.

## Overview

CrossTerm plugins extend terminal functionality through a sandboxed WebAssembly (WASM) runtime. Plugins can:

- **Add sidebar panels** — Custom UI panels rendered in the sidebar region, providing monitoring dashboards, session metadata viewers, or interactive controls.
- **React to SSH output** — Intercept and process terminal output streams for pattern detection, alerting, or logging.
- **Process terminal lines** — Transform, annotate, or highlight individual output lines before rendering.
- **Register custom commands** — Extend the command palette with plugin-provided actions accessible via keyboard shortcuts or menus.

Plugins run inside a WASI sandbox with explicit capability grants. They cannot access the host filesystem, network, or other system resources unless the user grants the corresponding permission.

## Plugin Manifest

Every plugin requires a `plugin.toml` manifest at its root:

```toml
[plugin]
name = "my-plugin"
version = "0.1.0"
author = "Your Name <you@example.com>"
description = "A brief description of what this plugin does."
license = "MIT"
min_crossterm_version = "1.0.0"

[permissions]
terminal_read = true
terminal_write = false
ssh_metadata = true
filesystem_read = false
network_outbound = false
```

| Field                  | Required | Description                                  |
|------------------------|----------|----------------------------------------------|
| `name`                 | Yes      | Unique plugin identifier (lowercase, hyphens)|
| `version`              | Yes      | SemVer version string                        |
| `author`               | Yes      | Author name and optional email               |
| `description`          | No       | Short description shown in the plugin list   |
| `license`              | No       | SPDX license identifier                      |
| `min_crossterm_version`| No       | Minimum CrossTerm version required           |

## Lifecycle Hooks

Plugins implement lifecycle hooks that CrossTerm invokes at specific events:

| Hook              | Trigger                              | Arguments                     |
|-------------------|--------------------------------------|-------------------------------|
| `on_connect`      | SSH/terminal session established     | `session_id`, `host`, `port`  |
| `on_disconnect`   | Session closed or connection lost     | `session_id`, `reason`        |
| `on_output_line`  | Each line of terminal output         | `session_id`, `line`          |
| `on_command`      | User executes a registered command   | `command_name`, `args`        |
| `on_tab_open`     | A new tab is opened                  | `tab_id`, `session_id`        |
| `on_tab_close`    | A tab is closed                      | `tab_id`                      |

Hooks are invoked asynchronously. If a hook returns an error, CrossTerm logs the failure and continues without crashing.

## Permission Model

Plugins declare required capabilities in their manifest. Users review and approve these during installation.

| Capability          | Grants                                               |
|---------------------|------------------------------------------------------|
| `terminal:read`     | Read terminal output streams                         |
| `terminal:write`    | Write input to terminal sessions                     |
| `ssh:metadata`      | Access SSH session metadata (host, user, port)       |
| `filesystem:read`   | Read files from the local filesystem (scoped paths)  |
| `network:outbound`  | Make HTTP/TCP requests to external services           |

Capabilities follow the principle of least privilege. A plugin requesting `terminal:write` will show a prominent security warning during installation. Plugins cannot escalate permissions after installation — manifest changes require user re-approval.

## Example Plugin

A minimal Rust plugin that logs SSH commands:

```rust
use crossterm_plugin_sdk::{Plugin, HookResult, SessionInfo};

pub struct CommandLogger;

impl Plugin for CommandLogger {
    fn name(&self) -> &str {
        "command-logger"
    }

    fn on_connect(&mut self, session: &SessionInfo) -> HookResult {
        log::info!(
            "Connected to {}@{}:{}",
            session.user, session.host, session.port
        );
        HookResult::Ok
    }

    fn on_output_line(&mut self, session_id: &str, line: &str) -> HookResult {
        if line.starts_with('$') || line.starts_with('#') {
            log::info!("[{}] cmd: {}", session_id, line);
        }
        HookResult::Ok
    }

    fn on_disconnect(&mut self, session_id: &str, reason: &str) -> HookResult {
        log::info!("Disconnected {}: {}", session_id, reason);
        HookResult::Ok
    }
}

crossterm_plugin_sdk::export_plugin!(CommandLogger);
```

## Plugin Cookbook

### Recipe 1: Syntax Highlighter

Apply ANSI color codes to recognized keywords in terminal output:

```rust
fn on_output_line(&mut self, _session_id: &str, line: &str) -> HookResult {
    let highlighted = line
        .replace("ERROR", "\x1b[31mERROR\x1b[0m")
        .replace("WARN", "\x1b[33mWARN\x1b[0m")
        .replace("OK", "\x1b[32mOK\x1b[0m");
    HookResult::Replace(highlighted)
}
```

### Recipe 2: Command Auto-Completer

Register a custom command that suggests completions based on command history:

```rust
fn on_command(&mut self, command_name: &str, args: &[&str]) -> HookResult {
    if command_name == "suggest" {
        let prefix = args.first().unwrap_or(&"");
        let matches: Vec<&str> = self.history
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .map(|s| s.as_str())
            .take(5)
            .collect();
        HookResult::Suggestions(matches.into_iter().map(String::from).collect())
    } else {
        HookResult::Ok
    }
}
```

### Recipe 3: Session Metrics Dashboard

Track connection durations and byte counts, exposed via a sidebar panel:

```rust
fn on_connect(&mut self, session: &SessionInfo) -> HookResult {
    self.sessions.insert(session.id.clone(), Instant::now());
    self.byte_counts.insert(session.id.clone(), 0u64);
    HookResult::Ok
}

fn on_output_line(&mut self, session_id: &str, line: &str) -> HookResult {
    if let Some(count) = self.byte_counts.get_mut(session_id) {
        *count += line.len() as u64;
    }
    HookResult::Ok
}

fn on_disconnect(&mut self, session_id: &str, _reason: &str) -> HookResult {
    if let Some(start) = self.sessions.remove(session_id) {
        let duration = start.elapsed();
        let bytes = self.byte_counts.remove(session_id).unwrap_or(0);
        log::info!("Session {} lasted {:?}, {} bytes", session_id, duration, bytes);
    }
    HookResult::Ok
}
```

## Building & Testing

Build your plugin targeting WASI:

```bash
cargo build --target wasm32-wasi --release
```

The output `.wasm` file is located at `target/wasm32-wasi/release/<plugin_name>.wasm`.

Run the test harness to validate hooks and permissions:

```bash
cargo install crossterm-plugin-test
crossterm-plugin-test ./target/wasm32-wasi/release/my_plugin.wasm
```

The test harness simulates lifecycle events, verifies permission boundaries, and checks for memory leaks or panics in the WASM sandbox.

For development iteration, use the CrossTerm plugin dev mode:

```bash
crossterm --plugin-dev ./path/to/plugin.wasm
```

This hot-reloads the plugin on file changes and displays hook invocations in the debug panel.

## Distribution

Publish your plugin to the CrossTerm plugin registry:

1. **Package** — Run `crossterm-plugin pack` to create a `.ctplugin` archive containing the WASM binary and manifest.
2. **Verify** — The packer runs automated security checks (permission audit, sandboxing validation).
3. **Submit** — Upload via `crossterm-plugin publish` or through the web portal at `plugins.crossterm.dev`.
4. **Review** — Plugins requesting sensitive permissions (`terminal:write`, `network:outbound`) undergo manual review before listing.

Users install plugins from within CrossTerm via **Settings → Plugins → Browse Registry** or the command palette (`Ctrl+Shift+P` → "Install Plugin").
