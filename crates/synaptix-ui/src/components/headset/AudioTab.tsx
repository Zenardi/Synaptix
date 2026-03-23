import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ToggleSwitch from "../ToggleSwitch";

interface Props {
  deviceId: string;
  pid: string;
}

export default function AudioTab({ deviceId, pid }: Props) {
  const [thxEnabled, setThxEnabled] = useState(false);
  const [sidetone, setSidetone] = useState(50);
  const [volume, setVolume] = useState<number | null>(null); // null = loading
  const [volumeError, setVolumeError] = useState<string | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const volumeDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Read actual system volume on mount so the slider starts in the right position.
  useEffect(() => {
    invoke<number>("get_volume", { deviceId })
      .then((v) => setVolume(Math.round(v)))
      .catch((err) => {
        console.warn("[AudioTab] get_volume failed:", err);
        setVolume(50); // sensible fallback
      });
  }, [deviceId]);

  const handleThx = (next: boolean) => {
    setThxEnabled(next);
    invoke("set_thx_spatial", { deviceId, pid, enabled: next }).catch((err) =>
      console.warn("[AudioTab] set_thx_spatial not implemented:", err),
    );
  };

  const handleSidetone = (value: number) => {
    setSidetone(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      invoke("set_sidetone", { deviceId, pid, level: value }).catch((err) =>
        console.warn("[AudioTab] set_sidetone not implemented:", err),
      );
    }, 300);
  };

  const handleVolume = (value: number) => {
    setVolume(value);
    setVolumeError(null);
    if (volumeDebounceRef.current) clearTimeout(volumeDebounceRef.current);
    volumeDebounceRef.current = setTimeout(() => {
      invoke<boolean>("set_volume", { deviceId, level: value }).catch((err) => {
        console.error("[AudioTab] set_volume error:", err);
        setVolumeError(String(err));
      });
    }, 150);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Output Volume */}
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium text-white">Output Volume</p>
          <span className="text-xs font-mono text-razer-green">
            {volume === null ? "…" : `${volume}%`}
          </span>
        </div>
        <p className="text-[11px] text-gray-500 -mt-1">
          Headset speaker volume via PipeWire
        </p>
        <input
          type="range"
          min={0}
          max={100}
          value={volume ?? 50}
          disabled={volume === null}
          onChange={(e) => handleVolume(Number(e.target.value))}
          className="w-full accent-razer-green cursor-pointer disabled:opacity-40"
          aria-label="Output volume"
        />
        <div className="flex justify-between text-[10px] text-gray-600">
          <span>Mute</span>
          <span>Max</span>
        </div>
        {volumeError && (
          <p className="text-[11px] text-red-400 mt-1">⚠ {volumeError}</p>
        )}
      </div>

      {/* THX Spatial Audio */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-white">THX Spatial Audio</p>
          <p className="text-[11px] text-gray-500 mt-0.5">
            360° virtual surround sound
          </p>
        </div>
        <ToggleSwitch
          enabled={thxEnabled}
          onChange={handleThx}
          label="THX Spatial Audio"
        />
      </div>

      {/* Sidetone Volume */}
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <p className="text-sm font-medium text-white">Sidetone Volume</p>
          <span className="text-xs font-mono text-razer-green">{sidetone}</span>
        </div>
        <p className="text-[11px] text-gray-500 -mt-1">
          Hear your own voice through the headset
        </p>
        <input
          type="range"
          min={0}
          max={100}
          value={sidetone}
          onChange={(e) => handleSidetone(Number(e.target.value))}
          className="w-full accent-razer-green cursor-pointer"
          aria-label="Sidetone volume"
        />
        <div className="flex justify-between text-[10px] text-gray-600">
          <span>Off</span>
          <span>Max</span>
        </div>
      </div>
    </div>
  );
}

