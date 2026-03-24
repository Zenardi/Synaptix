import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";

// ── Types ─────────────────────────────────────────────────────────────────────

type EffectMode = "Static" | "Breathing" | "Spectrum";

interface DeviceSettings {
  lighting?: { Static?: [number, number, number]; Breathing?: [number, number, number] } | "Spectrum";
}

// ── Constants ─────────────────────────────────────────────────────────────────

const MODES: EffectMode[] = ["Static", "Breathing", "Spectrum"];

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

// ── Helpers ───────────────────────────────────────────────────────────────────

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

function dispatchEffect(deviceId: string, mode: EffectMode, hex: string): void {
  let effect: unknown;
  if (mode === "Spectrum") {
    effect = "Spectrum";
  } else {
    const [r, g, b] = hexToRgb(hex);
    effect = { [mode]: [r, g, b] };
  }
  invoke("set_device_lighting", { deviceId, effect }).catch((err) =>
    console.warn("set_device_lighting:", err),
  );
}

// ── Component ─────────────────────────────────────────────────────────────────

interface Props {
  deviceId: string;
}

export default function LightingControl({ deviceId }: Props) {
  const [mode, setMode] = useState<EffectMode>("Static");
  const [selectedColor, setSelectedColor] = useState("#44d62c");

  // Hydrate lighting state from the daemon on mount so the UI reflects
  // the last saved colour/effect rather than always defaulting to green Static.
  useEffect(() => {
    invoke<string>("get_device_state", { deviceId })
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
  }, [deviceId]);

  const showColorPicker = mode !== "Spectrum";

  const applyColor = (hex: string) => {
    setSelectedColor(hex);
    dispatchEffect(deviceId, mode, hex);
  };

  const switchMode = (next: EffectMode) => {
    setMode(next);
    dispatchEffect(deviceId, next, selectedColor);
  };

  return (
    <div className="flex flex-col gap-4 max-w-sm">
      <p className="text-[10px] text-gray-500 tracking-widest uppercase">
        Effect
      </p>

      {/* Effect mode selector */}
      <div className="flex gap-1.5">
        {MODES.map((m) => (
          <button
            key={m}
            onClick={() => switchMode(m)}
            className={[
              "px-4 py-1.5 rounded-full text-[10px] font-semibold tracking-widest uppercase transition-all",
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
            className="overflow-hidden flex flex-col gap-3"
          >
            {/* Preset swatches */}
            <div className="grid grid-cols-8 gap-2">
              {PRESETS.map(({ label, hex }) => (
                <button
                  key={hex}
                  title={label}
                  onClick={() => applyColor(hex)}
                  className="w-7 h-7 rounded-full transition-transform hover:scale-110 focus:outline-none"
                  style={{
                    backgroundColor: hex,
                    boxShadow:
                      selectedColor.toLowerCase() === hex.toLowerCase()
                        ? `0 0 0 2px #111111, 0 0 0 3.5px ${hex}, 0 0 10px ${hex}`
                        : "none",
                  }}
                  aria-label={`Set colour ${label}`}
                  aria-pressed={selectedColor.toLowerCase() === hex.toLowerCase()}
                />
              ))}
            </div>

            {/* Custom hex input */}
            <div className="flex items-center gap-3">
              <input
                key={selectedColor}
                type="color"
                defaultValue={selectedColor}
                onInput={(e: React.FormEvent<HTMLInputElement>) =>
                  applyColor(e.currentTarget.value)
                }
                onChange={(e) => applyColor(e.target.value)}
                className="w-9 h-9 rounded cursor-pointer border-0 p-0"
                style={{ colorScheme: "dark" }}
                aria-label="Custom colour"
              />
              <span className="text-xs font-mono text-gray-400 tracking-wider uppercase select-all">
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
            className="text-[11px] text-gray-500"
          >
            Auto-cycling through all colours
          </motion.p>
        )}
      </AnimatePresence>
    </div>
  );
}
