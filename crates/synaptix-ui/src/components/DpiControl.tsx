import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

const DPI_PRESETS = [800, 1200, 1800, 2200, 2600];
const MIN_DPI = 500;
const DEBOUNCE_MS = 300;

interface Props {
  deviceId: string;
}

interface DeviceSettings {
  dpi?: number;
}

export default function DpiControl({ deviceId }: Props) {
  const [activeDpi, setActiveDpi] = useState<number>(800);
  const [customValue, setCustomValue] = useState<string>("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Hydrate from daemon on mount (or when device changes) so the UI reflects
  // the last saved configuration rather than defaulting to 800.
  useEffect(() => {
    invoke<string>("get_device_state", { deviceId })
      .then((json) => {
        const settings: DeviceSettings = JSON.parse(json);
        if (settings.dpi != null && settings.dpi >= MIN_DPI) {
          setActiveDpi(settings.dpi);
          setCustomValue("");
        }
      })
      .catch(() => {
        // No saved state — keep the default.
      });
  }, [deviceId]);

  const applyDpi = (dpi: number) => {
    invoke("set_device_dpi", { deviceId, dpi }).catch((err) =>
      console.warn("set_device_dpi:", err),
    );
  };

  const handlePreset = (dpi: number) => {
    setActiveDpi(dpi);
    setCustomValue("");
    applyDpi(dpi);
  };

  const handleCustomChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value;
    setCustomValue(raw);

    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      const parsed = parseInt(raw, 10);
      if (!isNaN(parsed) && parsed >= MIN_DPI) {
        setActiveDpi(parsed);
        applyDpi(parsed);
      }
    }, DEBOUNCE_MS);
  };

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  const displayDpi = customValue !== "" ? customValue : String(activeDpi);

  return (
    <div className="w-full border-t border-white/5 pt-4 flex flex-col gap-3">
      <p className="text-[10px] text-gray-500 tracking-widest uppercase text-center">
        DPI
      </p>

      {/* Preset buttons */}
      <div className="flex gap-1.5 justify-center flex-wrap">
        {DPI_PRESETS.map((dpi) => (
          <button
            key={dpi}
            onClick={() => handlePreset(dpi)}
            className={[
              "px-3 py-1 rounded-full text-[10px] font-semibold tracking-widest transition-all",
              activeDpi === dpi && customValue === ""
                ? "bg-[#44d62c] text-black shadow-[0_0_8px_#44d62c]"
                : "bg-white/5 text-gray-400 hover:bg-white/10",
            ].join(" ")}
          >
            {dpi}
          </button>
        ))}
      </div>

      {/* Custom DPI input */}
      <div className="flex items-center gap-2 justify-center">
        <input
          type="number"
          min={MIN_DPI}
          value={displayDpi}
          onChange={handleCustomChange}
          onFocus={() => setCustomValue(String(activeDpi))}
          className="w-24 bg-white/5 border border-white/10 rounded-md px-2 py-1 text-xs text-center text-white font-mono focus:outline-none focus:border-[#44d62c] transition-colors [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          aria-label="Custom DPI value"
        />
        <span className="text-[10px] text-gray-500 uppercase tracking-widest">
          DPI
        </span>
      </div>
    </div>
  );
}
