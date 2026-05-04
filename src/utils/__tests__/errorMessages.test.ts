import { describe, it, expect } from "vitest";
import { friendlyError, isErrorCode } from "@/utils/errorMessages";

describe("friendlyError", () => {
  it("auth_failed returns correct friendly string", () => {
    const result = friendlyError(JSON.stringify({ code: "auth_failed", message: "denied" }));
    expect(result).toBe(
      "Authentication failed. Check your username, password, or SSH key."
    );
  });

  it("host_unreachable includes the host from message field", () => {
    const result = friendlyError(JSON.stringify({ code: "host_unreachable", message: "example.com" }));
    expect(result).toContain("example.com");
    expect(result).toContain("Check the hostname, port, and your network connection.");
  });

  it("host_key_changed includes the fingerprint", () => {
    const result = friendlyError(
      JSON.stringify({ code: "host_key_changed", message: "x", fingerprint: "ab:cd:ef" })
    );
    expect(result).toContain("ab:cd:ef");
    expect(result).toContain("host key has changed");
  });

  it("host_key_changed uses 'unknown' when fingerprint is absent", () => {
    const result = friendlyError(JSON.stringify({ code: "host_key_changed", message: "x" }));
    expect(result).toContain("unknown");
  });

  it("connection_refused returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "connection_refused", message: "" }));
    expect(result).toBe(
      "Connection refused. Ensure the SSH service is running and the port is correct."
    );
  });

  it("connection_timeout returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "connection_timeout", message: "" }));
    expect(result).toBe(
      "Connection timed out. Check that the host is reachable and no firewall is blocking the port."
    );
  });

  it("vault_locked returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "vault_locked", message: "" }));
    expect(result).toBe(
      "The credential vault is locked. Please unlock it to continue."
    );
  });

  it("vault_wrong_password returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "vault_wrong_password", message: "" }));
    expect(result).toBe("Incorrect master password. Check Caps Lock and try again.");
  });

  it("vault_not_found returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "vault_not_found", message: "" }));
    expect(result).toBe("Vault not found. It may have been deleted or moved.");
  });

  it("rate_limited includes the retry_after_secs value", () => {
    const result = friendlyError(
      JSON.stringify({ code: "rate_limited", message: "slow", retry_after_secs: 30 })
    );
    expect(result).toContain("30");
  });

  it("rate_limited defaults to 60 seconds when retry_after_secs is absent", () => {
    const result = friendlyError(JSON.stringify({ code: "rate_limited", message: "" }));
    expect(result).toContain("60");
  });

  it("credential_not_found includes quoted id when present", () => {
    const result = friendlyError(
      JSON.stringify({ code: "credential_not_found", message: "", id: "my-cred" })
    );
    expect(result).toContain('"my-cred"');
  });

  it("credential_not_found omits id section when id is absent", () => {
    const result = friendlyError(JSON.stringify({ code: "credential_not_found", message: "" }));
    expect(result).toContain("not found");
  });

  it("permission_denied returns correct message", () => {
    const result = friendlyError(JSON.stringify({ code: "permission_denied", message: "" }));
    expect(result).toBe(
      "Permission denied. You do not have access to perform this action."
    );
  });

  it("not_found falls back to the message field when present", () => {
    const result = friendlyError(JSON.stringify({ code: "not_found", message: "Profile missing" }));
    expect(result).toBe("Profile missing");
  });

  it("not_found uses generic text when message is empty", () => {
    const result = friendlyError(JSON.stringify({ code: "not_found", message: "" }));
    expect(result).toBe("The requested resource was not found.");
  });

  it("io_error includes message and disk space hint", () => {
    const result = friendlyError(JSON.stringify({ code: "io_error", message: "read-only fs" }));
    expect(result).toContain("read-only fs");
    expect(result).toContain("Check disk space and permissions.");
  });

  it("invalid_input falls back to message field", () => {
    const result = friendlyError(JSON.stringify({ code: "invalid_input", message: "Port must be 1-65535" }));
    expect(result).toBe("Port must be 1-65535");
  });

  it("invalid_input uses generic text when message is empty", () => {
    const result = friendlyError(JSON.stringify({ code: "invalid_input", message: "" }));
    expect(result).toBe("Invalid input. Please check the values and try again.");
  });

  it("internal returns restart message", () => {
    const result = friendlyError(JSON.stringify({ code: "internal", message: "" }));
    expect(result).toContain("unexpected error");
    expect(result).toContain("restart CrossTerm");
  });

  it("unknown code falls back to payload message", () => {
    const result = friendlyError(JSON.stringify({ code: "some_unknown_code", message: "Custom error text" }));
    expect(result).toBe("Custom error text");
  });

  it("unknown code with empty message falls back to internal message", () => {
    const result = friendlyError(JSON.stringify({ code: "some_unknown_code", message: "" }));
    expect(result).toContain("unexpected error");
  });

  it("plain string error returns the string unchanged", () => {
    const result = friendlyError("plain string error");
    expect(result).toBe("plain string error");
  });

  it("null input returns the internal error message", () => {
    const result = friendlyError(null);
    expect(result).toContain("unexpected error");
  });

  it("undefined input returns the internal error message", () => {
    const result = friendlyError(undefined);
    expect(result).toContain("unexpected error");
  });

  it("{bad json} string returns the raw string", () => {
    const result = friendlyError("{bad json}");
    expect(result).toBe("{bad json}");
  });

  it("object with code property is handled directly", () => {
    const result = friendlyError({ code: "vault_locked", message: "" });
    expect(result).toBe("The credential vault is locked. Please unlock it to continue.");
  });

  it("object without code falls through to internal", () => {
    const result = friendlyError({ message: "something went wrong" });
    expect(result).toContain("unexpected error");
  });
});

describe("isErrorCode", () => {
  it("returns true when code matches", () => {
    expect(
      isErrorCode(JSON.stringify({ code: "vault_locked", message: "" }), "vault_locked")
    ).toBe(true);
  });

  it("returns false when code does not match", () => {
    expect(
      isErrorCode(JSON.stringify({ code: "vault_locked", message: "" }), "auth_failed")
    ).toBe(false);
  });

  it("returns false for non-string input", () => {
    expect(isErrorCode({ code: "vault_locked", message: "" }, "vault_locked")).toBe(false);
  });

  it("returns false for invalid JSON string", () => {
    expect(isErrorCode("{bad json}", "vault_locked")).toBe(false);
  });

  it("returns false when input is null", () => {
    expect(isErrorCode(null, "vault_locked")).toBe(false);
  });

  it("returns true for rate_limited code check", () => {
    expect(
      isErrorCode(JSON.stringify({ code: "rate_limited", message: "", retry_after_secs: 30 }), "rate_limited")
    ).toBe(true);
  });

  it("returns true for auth_failed code check", () => {
    expect(
      isErrorCode(JSON.stringify({ code: "auth_failed", message: "denied" }), "auth_failed")
    ).toBe(true);
  });
});
