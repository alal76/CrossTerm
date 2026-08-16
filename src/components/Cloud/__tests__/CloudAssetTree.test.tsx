import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import CloudAssetTree from "@/components/Cloud/CloudAssetTree";
import { invoke } from "@tauri-apps/api/core";
import type { CloudAssetNode } from "@/types";

const mockInvoke = vi.mocked(invoke);

function node(overrides: Partial<CloudAssetNode> = {}): CloudAssetNode {
  return {
    id: "n1",
    name: "us-east-1",
    node_type: "region",
    provider: "aws",
    children: [],
    metadata: {},
    ...overrides,
  };
}

describe("CloudAssetTree", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and renders the asset tree", async () => {
    mockInvoke.mockResolvedValue([node()]);
    render(<CloudAssetTree />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_get_asset_tree"));
    expect(await screen.findByText("us-east-1")).toBeInTheDocument();
  });

  it("shows the empty state when there are no resources", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<CloudAssetTree />);
    expect(await screen.findByText(/No resources found/)).toBeInTheDocument();
  });

  it("expands to show nested children and collapses on click", async () => {
    mockInvoke.mockResolvedValue([
      node({
        id: "n1",
        name: "us-east-1",
        node_type: "region",
        children: [node({ id: "n2", name: "i-12345", node_type: "compute" })],
      }),
    ]);
    render(<CloudAssetTree />);
    expect(await screen.findByText("i-12345")).toBeInTheDocument();

    fireEvent.click(screen.getByText("us-east-1"));
    expect(screen.queryByText("i-12345")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("us-east-1"));
    expect(await screen.findByText("i-12345")).toBeInTheDocument();
  });

  it("shows a state badge on nodes with metadata.state", async () => {
    mockInvoke.mockResolvedValue([node({ metadata: { state: "running" } })]);
    render(<CloudAssetTree />);
    expect(await screen.findByText("running")).toBeInTheDocument();
  });

  it("refresh button reloads the tree", async () => {
    mockInvoke.mockResolvedValue([node()]);
    render(<CloudAssetTree />);
    await screen.findByText("us-east-1");
    mockInvoke.mockClear();

    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_get_asset_tree"));
  });
});
