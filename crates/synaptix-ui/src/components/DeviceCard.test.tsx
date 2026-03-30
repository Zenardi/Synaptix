/**
 * DeviceCard.test.tsx
 *
 * Integration tests for the full battery-display pipeline:
 *   invoke("get_razer_devices") → initial render → listen("device-battery-updated")
 *   → event callback fired → React state update → DeviceCard shows new level.
 *
 * The App is rendered (rather than DeviceCard in isolation) because that is
 * where the Tauri `listen` subscription lives.
 *
 * Coverage:
 *   - Mouse with known Discharging battery (existing tests)
 *   - Headset with Unknown battery: renders ?, no %, aria-label correct
 *   - Unknown → Discharging transition: ? disappears, real % appears
 *   - Unknown + Bluetooth: no spurious "Charging" badge
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
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
  capabilities: ["BatteryReporting"],
  connection_type: "Wired",
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
    render(<MemoryRouter><App /></MemoryRouter>);

    await waitFor(() =>
      expect(screen.getByText("75%")).toBeInTheDocument(),
    );
    expect(screen.getByText("Razer DeathAdder V2 Pro")).toBeInTheDocument();
  });

  it("updates DeviceCard when device-battery-updated event fires at 15%", async () => {
    render(<MemoryRouter><App /></MemoryRouter>);

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
    render(<MemoryRouter><App /></MemoryRouter>);
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

// ── Headset: Unknown battery state ───────────────────────────────────────────

const HEADSET_UNKNOWN: RazerDevice = {
  device_id: "kraken-v4-pro-0568",
  name: "Razer Kraken V4 Pro",
  product_id: "KrakenV4Pro",
  battery_state: "Unknown",
  capabilities: ["BatteryReporting"],
  connection_type: "Bluetooth",
};

describe("DeviceCard: Unknown battery state (headset)", () => {
  let capturedBatteryCallback: BatteryEventCallback | undefined;

  beforeEach(() => {
    capturedBatteryCallback = undefined;

    vi.mocked(invoke).mockResolvedValue([HEADSET_UNKNOWN]);

    vi.mocked(listen).mockImplementation(
      (eventName: string, callback: unknown) => {
        if (eventName === "device-battery-updated") {
          capturedBatteryCallback = callback as BatteryEventCallback;
        }
        return Promise.resolve(() => { /* no-op unlisten */ });
      },
    );
  });

  it("renders ? when battery_state is Unknown", async () => {
    render(<MemoryRouter><App /></MemoryRouter>);

    await waitFor(() =>
      expect(screen.getByText("?")).toBeInTheDocument(),
    );

    // No percentage must be shown alongside the ?
    expect(screen.queryByText(/\d+%/)).not.toBeInTheDocument();

    // aria-label must signal the unknown state for accessibility
    expect(
      screen.getByLabelText("Battery level unknown"),
    ).toBeInTheDocument();
  });

  it("replaces ? with 62% when device-battery-updated fires with Discharging:62", async () => {
    render(<MemoryRouter><App /></MemoryRouter>);

    // Wait for initial Unknown render
    await waitFor(() =>
      expect(screen.getByText("?")).toBeInTheDocument(),
    );
    expect(capturedBatteryCallback).toBeDefined();

    // Simulate the daemon resolving the battery level via D-Bus → Tauri event
    await act(async () => {
      capturedBatteryCallback!({
        payload: {
          device_id: "kraken-v4-pro-0568",
          battery_state: { Discharging: 62 },
        },
      });
    });

    // ? must be gone; real percentage must appear
    await waitFor(() =>
      expect(screen.getByText("62%")).toBeInTheDocument(),
    );
    expect(screen.queryByText("?")).not.toBeInTheDocument();
  });

  it("does not show a Charging badge when battery is Unknown and connection is Bluetooth", async () => {
    render(<MemoryRouter><App /></MemoryRouter>);

    await waitFor(() =>
      expect(screen.getByText("?")).toBeInTheDocument(),
    );

    // Unknown + Bluetooth must never show a spurious Charging badge
    expect(screen.queryByText("Charging")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("USB charging active")).not.toBeInTheDocument();
  });
});

// ── Wired keyboard: no battery ───────────────────────────────────────────────
// A device without the BatteryReporting capability must NOT render the
// battery ring at all — not even a "?" placeholder.

const WIRED_KEYBOARD: RazerDevice = {
  device_id: "blackwidow-v3-mini",
  name: "Razer BlackWidow V3 Mini HyperSpeed (Wired)",
  product_id: "BlackWidowV3MiniHyperSpeedWired",
  battery_state: "Unknown",
  capabilities: [{ Lighting: "Off" }],  // no BatteryReporting
  connection_type: "Wired",
};

describe("DeviceCard: no battery ring for wired keyboard (no BatteryReporting)", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue([WIRED_KEYBOARD]);
    vi.mocked(listen).mockImplementation(() =>
      Promise.resolve(() => { /* no-op unlisten */ }),
    );
  });

  it("does not render a battery ring or ? for a device without BatteryReporting", async () => {
    render(<MemoryRouter><App /></MemoryRouter>);

    await waitFor(() =>
      expect(screen.getByText("Razer BlackWidow V3 Mini HyperSpeed (Wired)")).toBeInTheDocument(),
    );

    // No "?" and no percentage — the ring should be completely absent
    expect(screen.queryByText("?")).not.toBeInTheDocument();
    expect(screen.queryByText(/\d+%/)).not.toBeInTheDocument();

    // No battery aria-label should be present
    expect(screen.queryByLabelText(/battery/i)).not.toBeInTheDocument();
  });
});
