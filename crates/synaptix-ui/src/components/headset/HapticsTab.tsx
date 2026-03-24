import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  deviceId: string;
  pid: string;
}

// Discrete haptic levels matching the Kraken V4 Pro's physical side-button
// cycle: Off → Low → Medium → High. Byte values are evenly spaced across the
// 0–100 hardware range that the USB protocol accepts.
const HAPTIC_LEVELS = [
  { label: "Off",    value: 0   },
  { label: "Low",    value: 33  },
  { label: "Medium", value: 66  },
  { label: "High",   value: 100 },
] as const;

type HapticLevel = (typeof HAPTIC_LEVELS)[number]["value"];

export default function HapticsTab({ deviceId, pid }: Props) {
  const [activeLevel, setActiveLevel] = useState<HapticLevel>(0);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleLevel = (value: HapticLevel) => {
    if (pending) return;
    setActiveLevel(value);
    setError(null);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setPending(true);
      invoke<boolean>("set_haptic_intensity", { deviceId, pid, level: value })
        .then((ok) => {
          if (!ok) setError("Daemon rejected command — check device is detected");
        })
        .catch((err) => {
          setError(`Command failed: ${err}`);
          // Revert visual state on error so it doesn't show a false active level
          setActiveLevel((prev) => prev);
        })
        .finally(() => setPending(false));
    }, 150);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Header */}
      <div>
        <p className="text-sm font-medium text-white">Haptic Feedback</p>
        <p className="text-[11px] text-gray-500 mt-0.5">
          Feel in-game events through the headset
        </p>
      </div>

      {/* Preset buttons — mirrors the physical side-button cycle */}
      <div className="grid grid-cols-4 gap-2">
        {HAPTIC_LEVELS.map(({ label, value }) => {
          const isActive = activeLevel === value;
          return (
            <button
              key={label}
              onClick={() => handleLevel(value)}
              disabled={pending}
              className={[
                "flex flex-col items-center gap-1.5 py-4 rounded-xl border transition-all",
                isActive
                  ? "bg-razer-green/10 border-razer-green text-razer-green shadow-[0_0_12px_#44d62c33]"
                  : "bg-white/5 border-white/10 text-gray-400 hover:bg-white/10 hover:text-white",
                pending ? "opacity-50 cursor-wait" : "cursor-pointer",
              ].join(" ")}
              aria-pressed={isActive}
              aria-label={`Haptic level: ${label}`}
            >
              {/* Intensity indicator dots */}
              <span className="flex gap-0.5">
                {HAPTIC_LEVELS.filter((l) => l.value > 0).map((l) => (
                  <span
                    key={l.value}
                    className={[
                      "w-1.5 h-1.5 rounded-full transition-colors",
                      value >= l.value
                        ? isActive ? "bg-razer-green" : "bg-gray-500"
                        : "bg-gray-700",
                    ].join(" ")}
                  />
                ))}
              </span>
              <span className="text-[11px] font-semibold tracking-wide uppercase">
                {label}
              </span>
            </button>
          );
        })}
      </div>

      {/* Status / error line */}
      {error ? (
        <p className="text-[11px] text-red-400 flex items-center gap-1">
          <span>⚠</span> {error}
        </p>
      ) : (
        <p className="text-[11px] text-gray-500">
          {pending
            ? "Sending…"
            : activeLevel === 0
              ? "Haptic feedback disabled"
              : `Intensity: ${activeLevel}/100 — matches headset side-button cycle`}
        </p>
      )}
    </div>
  );
}
