/**
 * App.battery.mockipc.test.tsx
 *
 * Integration tests that exercise the REAL Tauri IPC layer using
 * `@tauri-apps/api/mocks` `mockIPC` / `shouldMockEvents`.
 *
 * These tests intentionally DO NOT use `vi.mock("@tauri-apps/api/core")` or
 * `vi.mock("@tauri-apps/api/event")`. Instead they set up
 * `window.__TAURI_INTERNALS__` so the real invoke/listen/emit code paths
 * are executed — identical to what runs inside a live Tauri window.
 *
 * Coverage:
 *   - invoke("get_razer_devices") → headset Unknown → renders ?
 *   - invoke returns headset Unknown → device-battery-updated event → renders 43%
 *   - invoke returns mouse Discharging:75 → renders 75%
 *   - invoke("get_razer_devices") is called exactly once on mount
 *   - error from invoke → "Daemon unavailable" message
 */

import { describe, it, expect, afterEach } from "vitest";
import { render, screen, waitFor, act, cleanup } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import "@testing-library/jest-dom/vitest";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import App from "./App";
import type { RazerDevice } from "./App";

// ── Fixtures ──────────────────────────────────────────────────────────────────

const HEADSET_UNKNOWN: RazerDevice = {
  device_id: "kraken-v4-pro-0568",
  name: "Razer Kraken V4 Pro",
  product_id: "KrakenV4Pro",
  battery_state: "Unknown",
  capabilities: [],
  connection_type: "Bluetooth",
};

const MOUSE_DISCHARGING: RazerDevice = {
  device_id: "da-v2-pro",
  name: "Razer DeathAdder V2 Pro",
  product_id: "DeathAdderV2Pro",
  battery_state: { Discharging: 75 },
  capabilities: [],
  connection_type: "Wired",
};

// ── Cleanup ───────────────────────────────────────────────────────────────────
// Unmount all rendered trees first (flushes async unlisten effects), THEN clear
// the Tauri IPC mocks. The order matters: clearMocks() deletes
// window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener, and the unlisten
// callbacks still in flight would throw if clearMocks ran first.
afterEach(async () => {
  await act(async () => {
    cleanup();
  });
  clearMocks();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("App battery display — mockIPC real IPC layer", () => {
  it("renders ? when daemon returns headset with Unknown battery", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "get_razer_devices") return [HEADSET_UNKNOWN];
        return null;
      },
      { shouldMockEvents: true },
    );

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText("?")).toBeInTheDocument());
    // No numeric percentage while state is Unknown
    expect(screen.queryByText(/\d+%/)).not.toBeInTheDocument();
    // Accessibility: aria-label signals unknown state
    expect(screen.getByLabelText("Battery level unknown")).toBeInTheDocument();
  });

  it("transitions from ? to 43% when device-battery-updated event fires", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "get_razer_devices") return [HEADSET_UNKNOWN];
        return null;
      },
      { shouldMockEvents: true },
    );

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );

    // Wait for the initial Unknown state to render.
    await waitFor(() => expect(screen.getByText("?")).toBeInTheDocument());

    // Simulate the D-Bus signal arriving from the daemon after it queries
    // the headset and gets a real battery reading.
    await act(async () => {
      await emit("device-battery-updated", {
        device_id: "kraken-v4-pro-0568",
        battery_state: { Discharging: 43 },
      });
    });

    // ? must be replaced by the real percentage.
    await waitFor(() => expect(screen.getByText("43%")).toBeInTheDocument());
    expect(screen.queryByText("?")).not.toBeInTheDocument();
  });

  it("renders 75% for mouse with Discharging:75 (smoke test)", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "get_razer_devices") return [MOUSE_DISCHARGING];
        return null;
      },
      { shouldMockEvents: true },
    );

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText("75%")).toBeInTheDocument());
    expect(screen.getByText("Razer DeathAdder V2 Pro")).toBeInTheDocument();
  });

  it("calls get_razer_devices exactly once on mount", async () => {
    let callCount = 0;

    mockIPC(
      (cmd) => {
        if (cmd === "get_razer_devices") {
          callCount++;
          return [];
        }
        return null;
      },
      { shouldMockEvents: true },
    );

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );

    // Wait for mount effects to flush; React.StrictMode double-invokes effects
    // in development but not in production/test mode.
    await waitFor(() => expect(callCount).toBeGreaterThanOrEqual(1));
    expect(callCount).toBe(1);
  });

  it("shows Daemon unavailable message when invoke rejects", async () => {
    mockIPC(
      (cmd) => {
        if (cmd === "get_razer_devices")
          throw new Error("D-Bus connection refused");
        return null;
      },
      { shouldMockEvents: true },
    );

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>,
    );

    await waitFor(() =>
      expect(screen.getByText(/Daemon unavailable/i)).toBeInTheDocument(),
    );
  });
});
