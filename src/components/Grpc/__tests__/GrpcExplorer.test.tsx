import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import GrpcExplorer from "@/components/Grpc/GrpcExplorer";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import type { GrpcConfig, GrpcService, GrpcRpcResult } from "@/types";

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: GrpcConfig = { endpoint: "http://10.0.0.40:50051", verify_tls: false, metadata: {} };

const serviceDesc: GrpcService = {
  name: "demo.Greeter",
  methods: [
    { name: "SayHello", client_streaming: false, server_streaming: false, input_type: ".demo.HelloRequest", output_type: ".demo.HelloResponse" },
    { name: "StreamThings", client_streaming: false, server_streaming: true, input_type: ".demo.Req", output_type: ".demo.Resp" },
  ],
};

describe("GrpcExplorer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects and lists services", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.resolve("conn-1");
      if (cmd === "grpc_list_services") return Promise.resolve(["demo.Greeter"]);
      return Promise.resolve(undefined);
    });

    renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText("demo.Greeter")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("grpc_list_services", { id: "conn-1" });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.reject(new Error("Transport error: connection refused"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't connect/)).toBeInTheDocument();
    expect(screen.getByText(/connection refused/)).toBeInTheDocument();
  });

  it("expands a service to show its methods and selects one", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.resolve("conn-1");
      if (cmd === "grpc_list_services") return Promise.resolve(["demo.Greeter"]);
      if (cmd === "grpc_describe_service") return Promise.resolve(serviceDesc);
      return Promise.resolve(undefined);
    });

    renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("demo.Greeter");

    fireEvent.click(screen.getByText("demo.Greeter"));

    expect(await screen.findByText("SayHello")).toBeInTheDocument();
    fireEvent.click(screen.getByText("SayHello"));

    expect(await screen.findByText(/.demo.HelloRequest/)).toBeInTheDocument();
  });

  it("invokes the selected unary method with the JSON body and shows the response", async () => {
    const result: GrpcRpcResult = { status_code: 0, message: "OK", body: '{\n  "message": "hi"\n}', trailing_metadata: {} };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.resolve("conn-1");
      if (cmd === "grpc_list_services") return Promise.resolve(["demo.Greeter"]);
      if (cmd === "grpc_describe_service") return Promise.resolve(serviceDesc);
      if (cmd === "grpc_invoke") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("demo.Greeter");
    fireEvent.click(screen.getByText("demo.Greeter"));
    fireEvent.click(await screen.findByText("SayHello"));

    const textarea = await screen.findByPlaceholderText("{}");
    fireEvent.change(textarea, { target: { value: '{"name":"Ada"}' } });
    fireEvent.click(screen.getByText("Invoke"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("grpc_invoke", {
        id: "conn-1",
        service: "demo.Greeter",
        method: "SayHello",
        jsonBody: '{"name":"Ada"}',
      });
    });
    expect(await screen.findByText(/status 0/)).toBeInTheDocument();
  });

  it("blocks invoking a streaming method", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.resolve("conn-1");
      if (cmd === "grpc_list_services") return Promise.resolve(["demo.Greeter"]);
      if (cmd === "grpc_describe_service") return Promise.resolve(serviceDesc);
      return Promise.resolve(undefined);
    });

    renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("demo.Greeter");
    fireEvent.click(screen.getByText("demo.Greeter"));
    fireEvent.click(await screen.findByText(/StreamThings/));

    expect(await screen.findByText(/Streaming methods aren't supported yet/)).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "grpc_connect") return Promise.resolve("conn-1");
      if (cmd === "grpc_list_services") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<GrpcExplorer sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("grpc_list_services", { id: "conn-1" }));
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("grpc_disconnect", { id: "conn-1" });
  });
});
