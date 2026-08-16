import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@/i18n";
import PluginSidebar from "@/components/Plugin/PluginSidebar";

describe("PluginSidebar", () => {
  it("shows the empty state with no panels", () => {
    render(<PluginSidebar />);
    expect(screen.getByText("Sidebar Panels")).toBeInTheDocument();
  });

  it("renders panel titles collapsed by default", () => {
    render(
      <PluginSidebar
        panels={[{ id: "p1", pluginId: "plug", title: "My Panel", content: <div>Panel body</div> }]}
      />,
    );
    expect(screen.getByText("My Panel")).toBeInTheDocument();
    expect(screen.queryByText("Panel body")).not.toBeInTheDocument();
  });

  it("expands and collapses a panel on click", () => {
    render(
      <PluginSidebar
        panels={[{ id: "p1", pluginId: "plug", title: "My Panel", content: <div>Panel body</div> }]}
      />,
    );
    fireEvent.click(screen.getByText("My Panel"));
    expect(screen.getByText("Panel body")).toBeInTheDocument();

    fireEvent.click(screen.getByText("My Panel"));
    expect(screen.queryByText("Panel body")).not.toBeInTheDocument();
  });

  it("tracks multiple panels' expanded state independently", () => {
    render(
      <PluginSidebar
        panels={[
          { id: "p1", pluginId: "plug", title: "Panel One", content: <div>Body One</div> },
          { id: "p2", pluginId: "plug", title: "Panel Two", content: <div>Body Two</div> },
        ]}
      />,
    );
    fireEvent.click(screen.getByText("Panel One"));
    expect(screen.getByText("Body One")).toBeInTheDocument();
    expect(screen.queryByText("Body Two")).not.toBeInTheDocument();
  });
});
