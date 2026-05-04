/**
 * Maps AppError `code` values (returned by Tauri invoke) to user-friendly
 * copy. The backend's error.rs defines these codes as snake_case strings.
 * Keep in sync with src-tauri/src/error.rs AppError variants.
 */

export interface AppErrorPayload {
  code: string;
  message: string;
  detail?: string;
  fingerprint?: string;
  retry_after_secs?: number;
  id?: string;
}

const ERROR_COPY: Record<string, (e: AppErrorPayload) => string> = {
  auth_failed: () =>
    "Authentication failed. Check your username, password, or SSH key.",
  host_unreachable: (e) =>
    `Cannot reach ${e.message}. Check the hostname, port, and your network connection.`,
  host_key_changed: (e) =>
    `The server's host key has changed (fingerprint: ${e.fingerprint ?? "unknown"}). This may indicate a server reinstall or a security threat. Verify with your administrator before reconnecting.`,
  connection_refused: () =>
    "Connection refused. Ensure the SSH service is running and the port is correct.",
  connection_timeout: () =>
    "Connection timed out. Check that the host is reachable and no firewall is blocking the port.",
  vault_locked: () =>
    "The credential vault is locked. Please unlock it to continue.",
  vault_wrong_password: () =>
    "Incorrect master password. Check Caps Lock and try again.",
  vault_not_found: () =>
    "Vault not found. It may have been deleted or moved.",
  rate_limited: (e) =>
    `Too many failed attempts. Please wait ${e.retry_after_secs ?? 60} seconds before trying again.`,
  credential_not_found: (e) =>
    `Credential ${e.id ? `"${e.id}"` : ""} not found. It may have been deleted.`,
  permission_denied: () =>
    "Permission denied. You do not have access to perform this action.",
  not_found: (e) =>
    e.message || "The requested resource was not found.",
  io_error: (e) =>
    `File system error: ${e.message}. Check disk space and permissions.`,
  invalid_input: (e) =>
    e.message || "Invalid input. Please check the values and try again.",
  internal: () =>
    "An unexpected error occurred. Please restart CrossTerm. If the problem persists, check Help → Troubleshooting.",
};

/**
 * Converts a raw Tauri invoke error into a user-facing message.
 *
 * Usage:
 *   try { await invoke(...) }
 *   catch (err) { setError(friendlyError(err)) }
 */
export function friendlyError(raw: unknown): string {
  if (!raw) return ERROR_COPY.internal({ code: "internal", message: "" });

  // Tauri serializes AppError as a JSON string when it reaches the frontend
  let payload: AppErrorPayload | null = null;
  if (typeof raw === "string") {
    try {
      payload = JSON.parse(raw) as AppErrorPayload;
    } catch {
      // Raw string error from old-style commands — return as-is
      return raw;
    }
  } else if (typeof raw === "object" && raw !== null && "code" in raw) {
    payload = raw as AppErrorPayload;
  }

  if (payload?.code) {
    const handler = ERROR_COPY[payload.code];
    if (handler) return handler(payload);
  }

  return payload?.message || ERROR_COPY.internal({ code: "internal", message: "" });
}

/**
 * Returns true if the error payload has the given code.
 * Useful for conditional handling: if (isErrorCode(err, "vault_locked")) ...
 */
export function isErrorCode(raw: unknown, code: string): boolean {
  if (typeof raw !== "string") return false;
  try {
    const p = JSON.parse(raw) as AppErrorPayload;
    return p?.code === code;
  } catch {
    return false;
  }
}
