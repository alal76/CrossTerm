import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import LocaleInstaller from "@/components/Settings/LocaleInstaller";
import type { LocaleInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);

function fakeFile(content: object, name = "custom.json"): File {
  const file = new File([JSON.stringify(content)], name, { type: "application/json" });
  return file;
}

describe("LocaleInstaller", () => {
  let clickSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    clickSpy = vi.spyOn(HTMLInputElement.prototype, "click").mockImplementation(function (this: HTMLInputElement) {
      // Simulate the user picking a file by firing the onchange handler
      // the component wired up, with a fake FileList.
      Object.defineProperty(this, "files", { value: [fakeFile({ "app.name": "CrossTerm" })], configurable: true });
      this.onchange?.({ target: this } as unknown as Event);
    });
  });

  afterEach(() => {
    clickSpy.mockRestore();
  });

  it("renders nothing when closed", () => {
    const { container } = render(<LocaleInstaller open={false} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the header and installed locales when open", () => {
    const installed: LocaleInfo[] = [{ code: "fr", name: "French", native_name: "Français", rtl: false, completeness: 90 }];
    render(<LocaleInstaller open onClose={vi.fn()} installedLocales={installed} />);
    expect(screen.getByText("Install Community Locale")).toBeInTheDocument();
    expect(screen.getByText("Français")).toBeInTheDocument();
  });

  it("close button calls onClose", () => {
    const onClose = vi.fn();
    render(<LocaleInstaller open onClose={onClose} />);
    fireEvent.click(document.querySelector("button")!);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("imports a file and shows a preview", async () => {
    render(<LocaleInstaller open onClose={vi.fn()} />);
    fireEvent.click(screen.getByText("Import from File"));

    await waitFor(() => {
      expect(screen.getByText(/Preview: custom/)).toBeInTheDocument();
    });
    expect(screen.getByText("app.name")).toBeInTheDocument();
  });

  // Regression coverage: handleInstall used to be a stub that just cleared
  // the preview ("In production, save to i18n directory and register with
  // i18next") without ever calling the backend — the file looked installed
  // but nothing was actually saved.
  it("installing a previewed locale calls l10n_import_translations and clears the preview", async () => {
    mockInvoke.mockResolvedValue(1);
    const installed: LocaleInfo[] = [
      { code: "de", name: "German", native_name: "Deutsch", rtl: false, completeness: 80 },
    ];
    render(<LocaleInstaller open onClose={vi.fn()} installedLocales={installed} />);
    fireEvent.click(screen.getByText("Import from File"));
    await screen.findByText(/Preview: custom/);

    fireEvent.click(screen.getByText("Install"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("l10n_import_translations", {
        locale: "de",
        data: JSON.stringify({ "app.name": "CrossTerm" }),
      });
    });
    await waitFor(() => {
      expect(screen.queryByText(/Preview: custom/)).not.toBeInTheDocument();
    });
  });

  it("defaults the target locale to the one matching the uploaded filename", async () => {
    mockInvoke.mockResolvedValue(1);
    const installed: LocaleInfo[] = [
      { code: "fr", name: "French", native_name: "Français", rtl: false, completeness: 90 },
      { code: "custom", name: "Custom", native_name: "Custom", rtl: false, completeness: 50 },
    ];
    render(<LocaleInstaller open onClose={vi.fn()} installedLocales={installed} />);
    fireEvent.click(screen.getByText("Import from File"));
    await screen.findByText(/Preview: custom/);

    expect(screen.getByRole("combobox")).toHaveValue("custom");
  });

  it("disables Install and does not call the backend when there is no target locale", async () => {
    render(<LocaleInstaller open onClose={vi.fn()} installedLocales={[]} />);
    fireEvent.click(screen.getByText("Import from File"));
    await screen.findByText(/Preview: custom/);

    expect(screen.getByText("Install")).toBeDisabled();
    fireEvent.click(screen.getByText("Install"));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("shows an error toast and keeps the preview when the import fails", async () => {
    mockInvoke.mockRejectedValue(new Error("unsupported locale"));
    const installed: LocaleInfo[] = [
      { code: "de", name: "German", native_name: "Deutsch", rtl: false, completeness: 80 },
    ];
    render(<LocaleInstaller open onClose={vi.fn()} installedLocales={installed} />);
    fireEvent.click(screen.getByText("Import from File"));
    await screen.findByText(/Preview: custom/);

    fireEvent.click(screen.getByText("Install"));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(screen.getByText(/Preview: custom/)).toBeInTheDocument();
  });
});
