import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@/i18n";
import SftpToolbar from "@/components/SftpBrowser/SftpToolbar";

describe("SftpToolbar", () => {
  it("calls onPreview and onSync", () => {
    const onPreview = vi.fn();
    const onSync = vi.fn();
    render(<SftpToolbar onPreview={onPreview} onSync={onSync} />);

    fireEvent.click(screen.getByText("Preview"));
    expect(onPreview).toHaveBeenCalledOnce();

    fireEvent.click(screen.getByText("Folder Sync"));
    expect(onSync).toHaveBeenCalledOnce();
  });

  it("disables preview and sync buttons when requested", () => {
    render(<SftpToolbar previewDisabled syncDisabled />);
    expect(screen.getByText("Preview").closest("button")).toBeDisabled();
    expect(screen.getByText("Folder Sync").closest("button")).toBeDisabled();
  });
});
