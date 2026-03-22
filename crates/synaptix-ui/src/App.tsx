import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import DeviceCard from "./components/DeviceCard";

// Mirror of the Rust BatteryState enum after serde serialisation.
// Serde serialises unit-like variants as plain strings and tuple variants
// as { "VariantName": value }.
export type BatteryState =
  | { Charging: number }
  | { Discharging: number }
  | "Full";

// Matches the DeviceEntry struct returned by the Tauri `get_razer_devices` command.
export interface RazerDevice {
  device_id: string;
  name: string;
  product_id: unknown;
  battery_state: BatteryState;
}

// Matches BatteryUpdatePayload emitted by the Tauri signal listener.
interface BatteryUpdatePayload {
  device_id: string;
  battery_state: BatteryState;
}

export function getBatteryLevel(state: BatteryState): number {
  if (state === "Full") return 100;
  if (typeof state === "object" && "Charging" in state) return state.Charging;
  if (typeof state === "object" && "Discharging" in state)
    return state.Discharging;
  return 0;
}

export function isCharging(state: BatteryState): boolean {
  return typeof state === "object" && "Charging" in state;
}

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

  return (
    <div className="min-h-screen bg-[#111111] text-white select-none">
      <div className="p-8">
        {/* Header */}
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
    </div>
  );
}

export default App;
