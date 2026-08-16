import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import LocaleSelector from "@/components/Settings/LocaleSelector";
import { invoke } from "@tauri-apps/api/core";
import type { LocaleInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);

function locale(overrides: Partial<LocaleInfo> = {}): LocaleInfo {
  return { code: "en", name: "English", native_name: "English", rtl: false, completeness: 1, ...overrides };
}

function mockAll(locales: LocaleInfo[], current = "en") {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "l10n_list_locales") return Promise.resolve(locales);
    if (cmd === "l10n_get_locale") return Promise.resolve(current);
    if (cmd === "l10n_set_locale") return Promise.resolve(undefined);
    return Promise.resolve(undefined);
  });
}

describe("LocaleSelector", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads locales and shows the current selection", async () => {
    mockAll([locale(), locale({ code: "fr", name: "French", native_name: "Français", completeness: 0.8 })]);
    render(<LocaleSelector />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("l10n_get_locale"));
    expect(await screen.findAllByText("English")).not.toHaveLength(0);
  });

  it("opens the dropdown and lists all locales with completeness", async () => {
    mockAll([locale(), locale({ code: "fr", name: "French", native_name: "Français", completeness: 0.8 })]);
    render(<LocaleSelector />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("l10n_get_locale"));

    fireEvent.click(screen.getByText("Language").parentElement!.querySelector("button")!);
    expect(await screen.findByText("Français")).toBeInTheDocument();
    expect(screen.getByText("80%")).toBeInTheDocument();
  });

  it("selects a different locale", async () => {
    mockAll([locale(), locale({ code: "fr", name: "French", native_name: "Français" })]);
    render(<LocaleSelector />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("l10n_get_locale"));

    fireEvent.click(screen.getByText("Language").parentElement!.querySelector("button")!);
    fireEvent.click(await screen.findByText("Français"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("l10n_set_locale", { locale: "fr" });
    });
  });

  it("shows an RTL badge for RTL locales", async () => {
    mockAll([locale({ code: "ar", name: "Arabic", native_name: "العربية", rtl: true })], "ar");
    render(<LocaleSelector />);
    expect(await screen.findByText("Right-to-left")).toBeInTheDocument();
  });

  it("shows the translation-completeness note when incomplete", async () => {
    mockAll([locale({ completeness: 0.5 })]);
    render(<LocaleSelector />);
    expect(await screen.findByText(/Translation completeness: 50%/)).toBeInTheDocument();
  });
});
