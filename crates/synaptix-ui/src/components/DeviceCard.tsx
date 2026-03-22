import { useState } from "react";
import { motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import type { RazerDevice } from "../App";
import { getBatteryLevel, isCharging } from "../App";

const RADIUS = 45;
const STROKE_WIDTH = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

// Curated Razer-style palette — ordered dark → vibrant.
const PRESETS = [
  { label: "Razer Green", hex: "#44d62c" },
  { label: "Cyan",        hex: "#00e5ff" },
  { label: "Blue",        hex: "#2979ff" },
  { label: "Purple",      hex: "#aa00ff" },
  { label: "Pink",        hex: "#ff4081" },
  { label: "Red",         hex: "#ff1744" },
  { label: "Orange",      hex: "#ff6d00" },
  { label: "White",       hex: "#ffffff" },
];

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

interface Props {
  device: RazerDevice;
}

export default function DeviceCard({ device }: Props) {
  const level = getBatteryLevel(device.battery_state);
  const charging = isCharging(device.battery_state);
  const targetOffset = CIRCUMFERENCE * (1 - level / 100);

  const [selectedColor, setSelectedColor] = useState("#44d62c");

  const applyColor = (hex: string) => {
    setSelectedColor(hex);
    const [r, g, b] = hexToRgb(hex);
    invoke("set_device_lighting", {
      deviceId: device.device_id,
      effect: { Static: [r, g, b] },
    }).catch((err) =>
      console.warn("set_device_lighting:", err),
    );
  };

  return (
    <div className="bg-[#181818] rounded-xl p-6 border border-white/5 flex flex-col items-center gap-5">

      {/* ── Battery ring ────────────────────────────────────────────── */}
      <div className="relative flex items-center justify-center">
        <svg
          className="-rotate-90"
          viewBox="0 0 100 100"
          width={128}
          height={128}
          aria-label={`Battery level ${level}%`}
        >
          <circle
            cx="50" cy="50" r={RADIUS}
            fill="none" stroke="#2a2a2a" strokeWidth={STROKE_WIDTH}
          />
          <motion.circle
            cx="50" cy="50" r={RADIUS}
            fill="none"
            stroke="#44d62c"
            strokeWidth={STROKE_WIDTH}
            strokeLinecap="round"
            strokeDasharray={CIRCUMFERENCE}
            initial={{ strokeDashoffset: CIRCUMFERENCE }}
            animate={{ strokeDashoffset: targetOffset }}
            transition={{ duration: 1.5, ease: "easeOut" }}
            style={{ filter: "drop-shadow(0 0 6px #44d62c)" }}
          />
        </svg>

        <div className="absolute inset-0 flex flex-col items-center justify-center gap-0.5">
          <span className="text-2xl font-bold leading-none text-white">
            {level}%
          </span>
          {charging && (
            <span className="text-[9px] font-semibold tracking-widest uppercase text-razer-green">
              Charging
            </span>
          )}
        </div>
      </div>

      {/* ── Device name ──────────────────────────────────────────────── */}
      <p className="text-sm text-gray-300 font-medium text-center leading-snug">
        {device.name}
      </p>

      {/* ── Lighting colour picker ───────────────────────────────────── */}
      <div className="w-full border-t border-white/5 pt-4 flex flex-col gap-3">
        <p className="text-[10px] text-gray-500 tracking-widest uppercase text-center">
          Lighting
        </p>

        {/* Preset swatches */}
        <div className="grid grid-cols-8 gap-1.5 justify-items-center">
          {PRESETS.map(({ label, hex }) => (
            <button
              key={hex}
              title={label}
              onClick={() => applyColor(hex)}
              className="w-6 h-6 rounded-full transition-transform hover:scale-110 focus:outline-none"
              style={{
                backgroundColor: hex,
                boxShadow:
                  selectedColor.toLowerCase() === hex.toLowerCase()
                    ? `0 0 0 2px #111111, 0 0 0 3.5px ${hex}, 0 0 8px ${hex}`
                    : "none",
              }}
              aria-label={`Set colour ${label}`}
              aria-pressed={selectedColor.toLowerCase() === hex.toLowerCase()}
            />
          ))}
        </div>

        {/* Custom hex input */}
        <div className="flex items-center gap-2">
          <input
            key={selectedColor}
            type="color"
            defaultValue={selectedColor}
            onInput={(e: React.FormEvent<HTMLInputElement>) =>
              applyColor(e.currentTarget.value)
            }
            onChange={(e) => applyColor(e.target.value)}
            className="w-8 h-8 rounded cursor-pointer border-0 p-0"
            style={{ colorScheme: "dark" }}
            aria-label="Custom colour"
          />
          <span className="text-xs font-mono text-gray-500 tracking-wider uppercase select-all">
            {selectedColor.toUpperCase()}
          </span>
        </div>
      </div>
    </div>
  );
}
