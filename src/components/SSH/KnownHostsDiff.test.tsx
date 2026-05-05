import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import KnownHostsDiff, { type HostKeyChange } from "./KnownHostsDiff";

const baseChange: HostKeyChange = {
  host: "example.com",
  port: 22,
  oldFingerprint: "SHA256:AAAA1111",
  newFingerprint: "SHA256:BBBB2222",
  oldKeyType: "ssh-rsa",
  newKeyType: "ssh-ed25519",
  detectedAt: "2026-05-05T12:00:00Z",
};

describe("KnownHostsDiff", () => {
  it("renders host and port in the warning", () => {
    render(
      <KnownHostsDiff
        change={baseChange}
        onAccept={vi.fn()}
        onReject={vi.fn()}
        onForget={vi.fn()}
      />
    );
    expect(
      screen.getByText(/Host key mismatch detected for example\.com:22/)
    ).toBeTruthy();
  });

  it("Accept button calls onAccept with the change object", async () => {
    const onAccept = vi.fn();
    render(
      <KnownHostsDiff
        change={baseChange}
        onAccept={onAccept}
        onReject={vi.fn()}
        onForget={vi.fn()}
      />
    );
    await userEvent.click(screen.getByRole("button", { name: /accept new key/i }));
    expect(onAccept).toHaveBeenCalledOnce();
    expect(onAccept).toHaveBeenCalledWith(baseChange);
  });

  it("Forget button calls onForget with the host string", async () => {
    const onForget = vi.fn();
    render(
      <KnownHostsDiff
        change={baseChange}
        onAccept={vi.fn()}
        onReject={vi.fn()}
        onForget={onForget}
      />
    );
    await userEvent.click(screen.getByRole("button", { name: /forget old key/i }));
    expect(onForget).toHaveBeenCalledOnce();
    expect(onForget).toHaveBeenCalledWith("example.com");
  });
});
