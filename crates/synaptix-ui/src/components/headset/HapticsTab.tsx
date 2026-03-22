import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ToggleSwitch from "../ToggleSwitch";

interface Props {
  deviceId: string;
  pid: string;
}

export default function HapticsTab({ deviceId, pid }: Props) {
  const [enabled, setEnabled] = useState(false);
  const [intensity, setIntensity] = useState(50);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleToggle = (next: boolean) => {
    setEnabled(next);
    invoke("set_haptics_enabled", { deviceId, pid, enabled: next }).catch(
      (err) => console.warn("[HapticsTab] set_haptics_enabled not implemented:", err),
    );
  };

  const handleIntensity = (value: number) => {
    setIntensity(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      invoke("set_haptic_intensity", { deviceId, pid, level: value }).catch(
        (err) =>
          console.warn("[HapticsTab] set_haptic_intensity not implemented:", err),
      );
    }, 300);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Haptic Feedback Enable */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-white">Haptic Feedback</p>
          <p className="text-[11px] text-gray-500 mt-0.5">
            Feel in-game events through the headset
          </p>
        </div>
        <ToggleSwitch
          enabled={enabled}
          onChange={handleToggle}
          label="Haptic Feedback"
        />
      </div>

      {/* Haptic Intensity Slider — disabled when haptics off */}
      <div className={["flex flex-col gap-2", !enabled ? "opacity-40 pointer-events-none" : ""].join(" ")}>
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium text-white">Intensity</p>
          <span className="text-xs font-mono text-razer-green">{intensity}</span>
        </div>
        <p className="text-[11px] text-gray-500 -mt-1">
          Vibration strength (0 = subtle, 100 = strong)
        </p>
        <input
          type="range"
          min={0}
          max={100}
          value={intensity}
          onChange={(e) => handleIntensity(Number(e.target.value))}
          disabled={!enabled}
          className="w-full accent-razer-green cursor-pointer disabled:cursor-not-allowed"
          aria-label="Haptic intensity"
        />
        <div className="flex justify-between text-[10px] text-gray-600">
          <span>Subtle</span>
          <span>Strong</span>
        </div>
      </div>
    </div>
  );
}
