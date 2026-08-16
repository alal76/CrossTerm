import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import LocaleInstaller from "@/components/Settings/LocaleInstaller";
import type { LocaleInfo } from "@/types";

function fakeFile(content: object, name = "custom.json"): File {
  const file = new File([JSON.stringify(content)], name, { type: "application/json" });
  return file;
}

describe("LocaleInstaller", () => {
  let clickSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
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

  it("installing a previewed locale clears the preview", async () => {
    render(<LocaleInstaller open onClose={vi.fn()} />);
    fireEvent.click(screen.getByText("Import from File"));
    await screen.findByText(/Preview: custom/);

    fireEvent.click(screen.getByText("Install Plugin"));
    expect(screen.queryByText(/Preview: custom/)).not.toBeInTheDocument();
  });
});
