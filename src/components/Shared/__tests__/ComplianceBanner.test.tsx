import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ComplianceBanner from "@/components/Shared/ComplianceBanner";

describe("ComplianceBanner", () => {
  it("renders nothing when not visible", () => {
    const { container } = render(
      <ComplianceBanner isVisible={false} hostname="host1" sessionId="s1" />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows the recording notice with hostname", () => {
    render(<ComplianceBanner isVisible hostname="prod-1" sessionId="s1" />);
    expect(screen.getByText("prod-1")).toBeInTheDocument();
    expect(screen.getByText(/This session is being recorded/)).toBeInTheDocument();
  });

  it("does not show the stop-recording link by default", () => {
    render(<ComplianceBanner isVisible hostname="prod-1" sessionId="s1" />);
    expect(screen.queryByText("Stop recording")).not.toBeInTheDocument();
  });

  it("shows and wires up the stop-recording link when allowed", () => {
    const onDisableRecording = vi.fn();
    render(
      <ComplianceBanner
        isVisible
        hostname="prod-1"
        sessionId="s1"
        allowUserDisable
        onDisableRecording={onDisableRecording}
      />,
    );
    fireEvent.click(screen.getByText("Stop recording"));
    expect(onDisableRecording).toHaveBeenCalledOnce();
  });

  it("does not show the link when allowed but no callback is given", () => {
    render(<ComplianceBanner isVisible hostname="prod-1" sessionId="s1" allowUserDisable />);
    expect(screen.queryByText("Stop recording")).not.toBeInTheDocument();
  });
});
