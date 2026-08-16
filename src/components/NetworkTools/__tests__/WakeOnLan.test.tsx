import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import WakeOnLan from "@/components/NetworkTools/WakeOnLan";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

describe("WakeOnLan", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("sends a magic packet for a valid MAC address", async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<WakeOnLan />);

    fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), {
      target: { value: "AA:BB:CC:DD:EE:FF" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_wol_send", {
        target: { mac_address: "AA:BB:CC:DD:EE:FF", broadcast_ip: null },
      });
    });
    expect(await screen.findByText(/Magic packet sent to AA:BB:CC:DD:EE:FF/)).toBeInTheDocument();
  });

  it("includes a trimmed broadcast IP when provided", async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<WakeOnLan />);

    fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), {
      target: { value: "aa:bb:cc:dd:ee:ff" },
    });
    fireEvent.change(screen.getByPlaceholderText("255.255.255.255"), {
      target: { value: " 192.168.1.255 " },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_wol_send", {
        target: { mac_address: "aa:bb:cc:dd:ee:ff", broadcast_ip: "192.168.1.255" },
      });
    });
  });

  it("shows an error and does not call invoke for an invalid MAC address", async () => {
    render(<WakeOnLan />);

    fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), {
      target: { value: "not-a-mac" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/i }));

    expect(await screen.findByText(/Invalid MAC address/)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("shows a backend error message when the send fails", async () => {
    mockInvoke.mockRejectedValue(new Error("no such device"));
    render(<WakeOnLan />);

    fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), {
      target: { value: "AA:BB:CC:DD:EE:FF" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Send/i }));

    expect(await screen.findByText("no such device")).toBeInTheDocument();
  });

  it("submits on Enter key in the MAC field", async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<WakeOnLan />);

    const input = screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF");
    fireEvent.change(input, { target: { value: "AA:BB:CC:DD:EE:FF" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
  });

  it("disables the send button while empty", () => {
    render(<WakeOnLan />);
    expect(screen.getByRole("button", { name: /Send/i })).toBeDisabled();
  });
});
