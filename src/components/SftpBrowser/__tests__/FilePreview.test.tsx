import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import FilePreview from "@/components/SftpBrowser/FilePreview";
import { invoke } from "@tauri-apps/api/core";
import type { FilePreview as FilePreviewType } from "@/types";

const mockInvoke = vi.mocked(invoke);

function preview(overrides: Partial<FilePreviewType> = {}): FilePreviewType {
  return {
    path: "/home/user/notes.txt",
    content_type: "text/plain",
    data: "line one\nline two",
    size: 18,
    truncated: false,
    ...overrides,
  };
}

describe("FilePreview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and renders text content with line numbers", async () => {
    mockInvoke.mockResolvedValue(preview());
    render(
      <FilePreview sessionId="s1" path="/home/user/notes.txt" onClose={vi.fn()} onDownload={vi.fn()} />,
    );

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("sftp_preview", { sessionId: "s1", path: "/home/user/notes.txt" });
    });
    expect(await screen.findByText("line one")).toBeInTheDocument();
    expect(screen.getByText("line two")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
  });

  it("shows an error message when the preview fails", async () => {
    mockInvoke.mockRejectedValue(new Error("permission denied"));
    render(<FilePreview sessionId="s1" path="/x.bin" onClose={vi.fn()} onDownload={vi.fn()} />);
    expect(await screen.findByText(/permission denied/)).toBeInTheDocument();
  });

  it("renders an image with zoom controls", async () => {
    mockInvoke.mockResolvedValue(preview({ content_type: "image/png", data: "AAAA" }));
    render(<FilePreview sessionId="s1" path="/pic.png" onClose={vi.fn()} onDownload={vi.fn()} />);

    const img = await screen.findByAltText("pic.png");
    expect(img).toHaveAttribute("src", "data:image/png;base64,AAAA");

    fireEvent.click(screen.getByTitle("Zoom in"));
    expect(img.style.transform).toBe("scale(1.25)");
    fireEvent.click(screen.getByTitle("Zoom out"));
    fireEvent.click(screen.getByTitle("Zoom out"));
    expect(img.style.transform).toBe("scale(0.75)");
  });

  it("renders binary content as a raw block for other content types", async () => {
    mockInvoke.mockResolvedValue(preview({ content_type: "application/octet-stream", data: "\\x00\\x01" }));
    render(<FilePreview sessionId="s1" path="/blob.dat" onClose={vi.fn()} onDownload={vi.fn()} />);
    expect(await screen.findByText("\\x00\\x01")).toBeInTheDocument();
  });

  it("shows the truncated notice", async () => {
    mockInvoke.mockResolvedValue(preview({ truncated: true }));
    render(<FilePreview sessionId="s1" path="/big.txt" onClose={vi.fn()} onDownload={vi.fn()} />);
    expect(await screen.findByText(/File truncated/)).toBeInTheDocument();
  });

  it("download and close buttons call their callbacks", async () => {
    mockInvoke.mockResolvedValue(preview());
    const onDownload = vi.fn();
    const onClose = vi.fn();
    render(<FilePreview sessionId="s1" path="/home/user/notes.txt" onClose={onClose} onDownload={onDownload} />);
    await screen.findByText("line one");

    fireEvent.click(screen.getByTitle("Download"));
    expect(onDownload).toHaveBeenCalledWith("/home/user/notes.txt");

    fireEvent.click(screen.getByTitle("Close Preview"));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
