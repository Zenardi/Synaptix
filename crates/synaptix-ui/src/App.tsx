import { createContext, useEffect, useState } from "react";
import { Routes, Route } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Titlebar from "./Titlebar";
import DeviceCard from "./components/DeviceCard";
import DeviceDetail from "./pages/DeviceDetail";

// Mirror of the Rust BatteryState enum after serde serialisation.
// Serde serialises unit-like variants as plain strings and tuple variants
// as { "VariantName": value }.
export type BatteryState =
  | { Charging: number }
  | { Discharging: number }
  | "Full";

// Matches the DeviceEntry struct returned by the Tauri `get_razer_devices` command.
export type ConnectionType = "Wired" | "Dongle" | "Bluetooth";

export interface RazerDevice {
  device_id: string;
  name: string;
  product_id: unknown;
  battery_state: BatteryState;
  capabilities: (string | Record<string, unknown>)[];
  connection_type: ConnectionType;
}

// Matches BatteryUpdatePayload emitted by the Tauri signal listener.
interface BatteryUpdatePayload {
  device_id: string;
  battery_state: BatteryState;
}

// Matches ConnectionUpdatePayload emitted by the Tauri signal listener.
interface ConnectionUpdatePayload {
  device_id: string;
  connection_type: ConnectionType;
}

export function getBatteryLevel(state: BatteryState): number {
  if (state === "Full") return 100;
  if (typeof state === "object" && "Charging" in state) return state.Charging;
  if (typeof state === "object" && "Discharging" in state)
    return state.Discharging;
  return 0;
}

export function isCharging(
  state: BatteryState,
  connectionType?: ConnectionType,
): boolean {
  if (connectionType === "Wired") return true;
  if (state === "Full") return true;
  return typeof state === "object" && "Charging" in state;
}

/** Returns true when a device's capabilities list includes the given name. */
export function hasCapability(
  device: RazerDevice,
  cap: string,
): boolean {
  return device.capabilities.some((c) =>
    typeof c === "string" ? c === cap : cap in c,
  );
}

// ── Devices context ──────────────────────────────────────────────────────────
// Shared across Dashboard and DeviceDetail so we don't re-fetch on navigation.

interface DevicesContextValue {
  devices: RazerDevice[];
  error: string | null;
}

export const DevicesContext = createContext<DevicesContextValue>({
  devices: [],
  error: null,
});

// ── App root ─────────────────────────────────────────────────────────────────

function App() {
  const [devices, setDevices] = useState<RazerDevice[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Initial load
  useEffect(() => {
    invoke<RazerDevice[]>("get_razer_devices")
      .then(setDevices)
      .catch((err: unknown) => setError(String(err)));
  }, []);

  // Real-time battery updates via D-Bus signal → Tauri event bridge
  useEffect(() => {
    const unlisten = listen<BatteryUpdatePayload>(
      "device-battery-updated",
      (event) => {
        const { device_id, battery_state } = event.payload;
        setDevices((prev) =>
          prev.map((d) =>
            d.device_id === device_id ? { ...d, battery_state } : d,
          ),
        );
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Real-time connection type updates (wired ↔ dongle ↔ bluetooth)
  useEffect(() => {
    const unlisten = listen<ConnectionUpdatePayload>(
      "device-connection-changed",
      (event) => {
        const { device_id, connection_type } = event.payload;
        setDevices((prev) =>
          prev.map((d) =>
            d.device_id === device_id ? { ...d, connection_type } : d,
          ),
        );
      },
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <DevicesContext.Provider value={{ devices, error }}>
      <div className="min-h-screen bg-[#111111] text-white select-none">
        <Titlebar />
        <Routes>
          {/* ── Dashboard ── */}
          <Route
            path="/"
            element={
              <div className="pt-9 p-8">
                <h1 className="text-2xl font-bold mb-1 tracking-widest uppercase text-razer-green">
                  Synaptix
                </h1>
                <p className="text-xs text-gray-500 mb-8 tracking-wider uppercase">
                  Device Manager
                </p>

                {error && (
                  <div className="text-red-400 text-sm mb-4 bg-red-900/20 p-3 rounded-md border border-red-900/40">
                    Daemon unavailable: {error}
                  </div>
                )}

                {devices.length === 0 && !error && (
                  <p className="text-gray-600 text-sm">No devices connected.</p>
                )}

                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
                  {devices.map((device) => (
                    <DeviceCard key={device.device_id} device={device} />
                  ))}
                </div>
              </div>
            }
          />

          {/* ── Device detail ── */}
          <Route path="/device/:deviceId" element={<DeviceDetail />} />
        </Routes>
      </div>
    </DevicesContext.Provider>
  );
}

export default App;
