/**
 * DeviceCard.test.tsx
 *
 * Integration test for the full battery-update reactivity pipeline:
 *   invoke("get_razer_devices") → initial render → listen("device-battery-updated")
 *   → event callback fired → React state update → DeviceCard shows new level.
 *
 * The App is rendered (rather than DeviceCard in isolation) because that is
 * where the Tauri `listen` subscription lives.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";
import App from "../App";
import type { RazerDevice, BatteryState } from "../App";

// ── Tauri module mocks ────────────────────────────────────────────────────────

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

// Import mocked versions for use in tests.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ── Helpers ───────────────────────────────────────────────────────────────────

interface BatteryEventPayload {
  device_id: string;
  battery_state: BatteryState;
}

type BatteryEventCallback = (event: { payload: BatteryEventPayload }) => void;

const INITIAL_DEVICE: RazerDevice = {
  device_id: "da-v2-pro",
  name: "Razer DeathAdder V2 Pro",
  product_id: "DeathAdderV2Pro",
  battery_state: { Discharging: 75 },
};

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("DeviceCard battery reactivity via Tauri events", () => {
  let capturedBatteryCallback: BatteryEventCallback | undefined;

  beforeEach(() => {
    capturedBatteryCallback = undefined;

    vi.mocked(invoke).mockResolvedValue([INITIAL_DEVICE]);

    // Capture the callback registered for "device-battery-updated" so we can
    // fire it manually in tests to simulate a D-Bus signal reaching the UI.
    vi.mocked(listen).mockImplementation(
      (eventName: string, callback: unknown) => {
        if (eventName === "device-battery-updated") {
          capturedBatteryCallback = callback as BatteryEventCallback;
        }
        return Promise.resolve(() => {
          /* no-op unlisten */
        });
      },
    );
  });

  it("renders the initial battery level fetched from the daemon", async () => {
    render(<App />);

    await waitFor(() =>
      expect(screen.getByText("75%")).toBeInTheDocument(),
    );
    expect(screen.getByText("Razer DeathAdder V2 Pro")).toBeInTheDocument();
  });

  it("updates DeviceCard when device-battery-updated event fires at 15%", async () => {
    render(<App />);

    // Wait for the initial device load (invoke resolves → setDevices → render).
    await waitFor(() =>
      expect(screen.getByText("75%")).toBeInTheDocument(),
    );

    // The listen callback should have been registered by now.
    expect(capturedBatteryCallback).toBeDefined();

    // Simulate the Tauri event that the D-Bus signal listener emits.
    await act(async () => {
      capturedBatteryCallback!({
        payload: {
          device_id: "da-v2-pro",
          battery_state: { Discharging: 15 },
        },
      });
    });

    // Assert the DeviceCard re-rendered with the new battery level.
    await waitFor(() =>
      expect(screen.getByText("15%")).toBeInTheDocument(),
    );
    expect(screen.queryByText("75%")).not.toBeInTheDocument();
  });

  it("does not update an unrelated device when a foreign device_id is received", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByText("75%")).toBeInTheDocument());

    await act(async () => {
      capturedBatteryCallback!({
        payload: {
          device_id: "some-other-device",
          battery_state: { Discharging: 10 },
        },
      });
    });

    // da-v2-pro should be unaffected.
    expect(screen.getByText("75%")).toBeInTheDocument();
    expect(screen.queryByText("10%")).not.toBeInTheDocument();
  });
});
