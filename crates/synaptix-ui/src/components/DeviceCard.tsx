import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import type { RazerDevice, ConnectionType } from "../App";
import { getBatteryLevel, isCharging } from "../App";
import DpiControl from "./DpiControl";

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

const CONNECTION_META: Record<ConnectionType, { icon: string; label: string; color: string }> = {
  Wired:     { icon: "⚡", label: "Wired",      color: "text-[#44d62c]" },
  Dongle:    { icon: "📡", label: "USB Dongle", color: "text-blue-400"  },
  Bluetooth: { icon: "🔵", label: "Bluetooth",  color: "text-sky-400"   },
};

type EffectMode = "Static" | "Breathing" | "Spectrum";

interface DeviceSettings {
  lighting?: { Static?: [number, number, number]; Breathing?: [number, number, number] } | "Spectrum";
  dpi?: number;
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
}

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

function dispatchEffect(
  deviceId: string,
  mode: EffectMode,
  hex: string,
): void {
  let effect: unknown;
  if (mode === "Spectrum") {
    effect = "Spectrum";
  } else {
    const [r, g, b] = hexToRgb(hex);
    effect = { [mode]: [r, g, b] };   // { Static: [...] } or { Breathing: [...] }
  }
  invoke("set_device_lighting", { deviceId, effect }).catch((err) =>
    console.warn("set_device_lighting:", err),
  );
}

interface Props {
  device: RazerDevice;
}

export default function DeviceCard({ device }: Props) {
  const navigate = useNavigate();
  const level = getBatteryLevel(device.battery_state);
  const charging = isCharging(device.battery_state, device.connection_type);
  const targetOffset = CIRCUMFERENCE * (1 - level / 100);

  const [mode, setMode] = useState<EffectMode>("Static");
  const [selectedColor, setSelectedColor] = useState("#44d62c");

  // Hydrate lighting state from the daemon on mount so the UI reflects
  // the last saved colour/effect rather than always defaulting to green Static.
  useEffect(() => {
    invoke<string>("get_device_state", { deviceId: device.device_id })
      .then((json) => {
        const settings: DeviceSettings = JSON.parse(json);
        if (!settings.lighting) return;
        if (settings.lighting === "Spectrum") {
          setMode("Spectrum");
          return;
        }
        if (typeof settings.lighting === "object") {
          if ("Static" in settings.lighting && settings.lighting.Static) {
            const [r, g, b] = settings.lighting.Static;
            setMode("Static");
            setSelectedColor(rgbToHex(r, g, b));
          } else if ("Breathing" in settings.lighting && settings.lighting.Breathing) {
            const [r, g, b] = settings.lighting.Breathing;
            setMode("Breathing");
            setSelectedColor(rgbToHex(r, g, b));
          }
        }
      })
      .catch(() => {
        // No saved lighting — keep defaults.
      });
  }, [device.device_id]);

  const MODES: EffectMode[] = ["Static", "Breathing", "Spectrum"];
  const showColorPicker = mode !== "Spectrum";

  const applyColor = (hex: string) => {
    setSelectedColor(hex);
    dispatchEffect(device.device_id, mode, hex);
  };

  const switchMode = (next: EffectMode) => {
    setMode(next);
    dispatchEffect(device.device_id, next, selectedColor);
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
            stroke={charging ? "#44d62c" : "#44d62c"}
            strokeWidth={STROKE_WIDTH}
            strokeLinecap="round"
            strokeDasharray={CIRCUMFERENCE}
            initial={{ strokeDashoffset: CIRCUMFERENCE }}
            animate={{
              strokeDashoffset: targetOffset,
              // Pulse the glow while charging.
              filter: charging
                ? [
                    "drop-shadow(0 0 4px #44d62c)",
                    "drop-shadow(0 0 12px #44d62c)",
                    "drop-shadow(0 0 4px #44d62c)",
                  ]
                : "drop-shadow(0 0 6px #44d62c)",
            }}
            transition={
              charging
                ? { strokeDashoffset: { duration: 1.5, ease: "easeOut" }, filter: { duration: 1.8, repeat: Infinity, ease: "easeInOut" } }
                : { duration: 1.5, ease: "easeOut" }
            }
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

      {/* ── Connection + charging badges ──────────────────────────────── */}
      <div className="flex items-center gap-2 flex-wrap justify-center">
        {(() => {
          const meta = CONNECTION_META[device.connection_type] ?? CONNECTION_META.Bluetooth;
          return (
            <span className={`text-[10px] font-semibold tracking-widest uppercase flex items-center gap-1 px-2 py-0.5 rounded-full bg-white/5 ${meta.color}`}>
              <span aria-hidden="true">{meta.icon}</span>
              {meta.label}
            </span>
          );
        })()}

        {/* Separate charging badge — visible whenever the cable is supplying power,
            even when the active gaming connection is the wireless dongle. */}
        <AnimatePresence>
          {charging && (
            <motion.span
              key="charging-badge"
              initial={{ opacity: 0, scale: 0.8 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.8 }}
              transition={{ duration: 0.2 }}
              className="text-[10px] font-semibold tracking-widest uppercase flex items-center gap-1 px-2 py-0.5 rounded-full bg-[#44d62c]/10 text-[#44d62c] border border-[#44d62c]/30"
              aria-label="USB charging active"
            >
              <span aria-hidden="true">⚡</span>
              Charging
            </motion.span>
          )}
        </AnimatePresence>
      </div>

      {/* ── Lighting section ─────────────────────────────────────────── */}
      <div className="w-full border-t border-white/5 pt-4 flex flex-col gap-3">
        <p className="text-[10px] text-gray-500 tracking-widest uppercase text-center">
          Lighting
        </p>

        {/* Effect mode selector */}
        <div className="flex gap-1.5 justify-center">
          {MODES.map((m) => (
            <button
              key={m}
              onClick={() => switchMode(m)}
              className={[
                "px-3 py-1 rounded-full text-[10px] font-semibold tracking-widest uppercase transition-all",
                mode === m
                  ? "bg-razer-green text-black shadow-[0_0_8px_#44d62c]"
                  : "bg-white/5 text-gray-400 hover:bg-white/10",
              ].join(" ")}
            >
              {m}
            </button>
          ))}
        </div>

        {/* Colour picker — shown for Static and Breathing */}
        <AnimatePresence>
          {showColorPicker && (
            <motion.div
              key="color-picker"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.2 }}
              className="overflow-hidden flex flex-col gap-2"
            >
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
            </motion.div>
          )}
        </AnimatePresence>

        {/* Spectrum description */}
        <AnimatePresence>
          {mode === "Spectrum" && (
            <motion.p
              key="spectrum-label"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="text-[10px] text-gray-500 text-center"
            >
              Auto-cycling through all colours
            </motion.p>
          )}
        </AnimatePresence>
      </div>

      {/* ── DPI section — only for devices with sensor/DPI capability ── */}
      {device.capabilities.some((c) => c === "DpiControl") && (
        <DpiControl deviceId={device.device_id} />
      )}

      {/* ── Configure button ─────────────────────────────────────────── */}
      <button
        onClick={() => navigate(`/device/${device.device_id}`)}
        className="w-full mt-1 py-2 rounded-lg text-[11px] font-semibold tracking-widest uppercase transition-all border border-white/10 text-gray-400 hover:border-razer-green/50 hover:text-razer-green hover:bg-razer-green/5"
      >
        Configure →
      </button>
    </div>
  );
}
