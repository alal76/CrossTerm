import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import "@/i18n";
import PluginCookbook from "@/components/Help/PluginCookbook";

describe("PluginCookbook", () => {
  it("renders the cookbook sections", () => {
    render(<PluginCookbook />);
    expect(screen.getByText("Plugin Cookbook")).toBeInTheDocument();
    expect(screen.getAllByText("manifest.json").length).toBeGreaterThan(0);
    expect(screen.getByText("Lifecycle Hooks")).toBeInTheDocument();
    expect(screen.getByText(/on_connect/)).toBeInTheDocument();
  });
});
