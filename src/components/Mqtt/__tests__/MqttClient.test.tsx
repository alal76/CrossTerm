import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import MqttClient from "@/components/Mqtt/MqttClient";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MqttConfig, MqttMessage } from "@/types";

// jsdom doesn't implement scrollIntoView (used to keep the live message log scrolled to the bottom).
Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: MqttConfig = {
  host: "192.168.0.8",
  port: 1883,
  client_id: "crossterm-test",
  keep_alive_secs: 30,
  use_tls: false,
  clean_session: true,
};

describe("MqttClient", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and shows the empty message log", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mqtt_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<MqttClient sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/No messages yet/)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("mqtt_connect", { config });
  });

  it("subscribes to a topic and renders it as a removable chip", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mqtt_connect") return Promise.resolve("conn-1");
      if (cmd === "mqtt_subscribe") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<MqttClient sessionId="sess-1" config={config} />);
    await screen.findByText(/No messages yet/);

    const input = screen.getByPlaceholderText(/Topic filter/);
    fireEvent.change(input, { target: { value: "sensors/temp" } });
    fireEvent.click(screen.getByText("Subscribe"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("mqtt_subscribe", { id: "conn-1", topic: "sensors/temp", qos: "AtMostOnce" });
    });
    expect(await screen.findByText("sensors/temp")).toBeInTheDocument();
  });

  it("appends incoming mqtt:message events to the live log, filtered by session", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mqtt_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<MqttClient sessionId="sess-1" config={config} />);
    await screen.findByText(/No messages yet/);

    const message: MqttMessage = { session_id: "conn-1", topic: "sensors/temp", payload: "21.5", qos: 0, retain: false };
    act(() => {
      handlers["mqtt:message"]({ payload: message });
    });

    expect(await screen.findByText("21.5")).toBeInTheDocument();

    // A message for a different connection must not appear.
    act(() => {
      handlers["mqtt:message"]({ payload: { ...message, session_id: "other-conn", payload: "99.9" } });
    });
    expect(screen.queryByText("99.9")).not.toBeInTheDocument();
  });

  it("publishes a message with the selected QoS and retain flag", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mqtt_connect") return Promise.resolve("conn-1");
      if (cmd === "mqtt_publish") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<MqttClient sessionId="sess-1" config={config} />);
    await screen.findByText(/No messages yet/);

    fireEvent.change(screen.getByPlaceholderText("Publish topic"), { target: { value: "cmd/lights" } });
    fireEvent.change(screen.getByPlaceholderText("Payload"), { target: { value: "on" } });
    fireEvent.click(screen.getByText("Retain"));
    fireEvent.click(screen.getByText("Publish"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("mqtt_publish", {
        id: "conn-1",
        topic: "cmd/lights",
        payload: "on",
        qos: "AtMostOnce",
        retain: true,
      });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mqtt_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<MqttClient sessionId="sess-1" config={config} />);
    await screen.findByText(/No messages yet/);
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("mqtt_disconnect", { id: "conn-1" });
  });
});
