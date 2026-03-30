/**
 * DaemonUnavailableScreen.test.tsx
 *
 * Unit tests for the DaemonUnavailableScreen component rendered when the
 * D-Bus ServiceUnknown error is detected (daemon not yet started after install).
 *
 * Coverage:
 *   - Renders a human-readable title (not a raw D-Bus error string)
 *   - Shows both required systemctl commands in visible code blocks
 *   - Renders a "Retry Connection" button
 *   - Calls the onRetry callback when the button is clicked
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import DaemonUnavailableScreen from "./DaemonUnavailableScreen";

describe("DaemonUnavailableScreen", () => {
  it("renders a human-readable title", () => {
    render(<DaemonUnavailableScreen onRetry={vi.fn()} />);
    expect(
      screen.getByText(/Synaptix Daemon is not running/i),
    ).toBeInTheDocument();
  });

  it("shows the daemon-reload systemctl command", () => {
    render(<DaemonUnavailableScreen onRetry={vi.fn()} />);
    expect(
      screen.getByText(/systemctl --user daemon-reload/i),
    ).toBeInTheDocument();
  });

  it("shows the start service systemctl command", () => {
    render(<DaemonUnavailableScreen onRetry={vi.fn()} />);
    expect(
      screen.getByText(/systemctl --user start synaptix-daemon\.service/i),
    ).toBeInTheDocument();
  });

  it("renders a Retry Connection button", () => {
    render(<DaemonUnavailableScreen onRetry={vi.fn()} />);
    expect(
      screen.getByRole("button", { name: /retry connection/i }),
    ).toBeInTheDocument();
  });

  it("calls onRetry when the Retry Connection button is clicked", () => {
    const onRetry = vi.fn();
    render(<DaemonUnavailableScreen onRetry={onRetry} />);
    fireEvent.click(screen.getByRole("button", { name: /retry connection/i }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
