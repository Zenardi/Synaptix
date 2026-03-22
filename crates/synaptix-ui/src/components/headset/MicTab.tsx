import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ToggleSwitch from "../ToggleSwitch";

interface Props {
  deviceId: string;
  pid: string;
}

export default function MicTab({ deviceId, pid }: Props) {
  const [muted, setMuted] = useState(false);

  const handleMute = (next: boolean) => {
    setMuted(next);
    invoke("set_mic_mute", { deviceId, pid, muted: next }).catch((err) =>
      console.warn("[MicTab] set_mic_mute not implemented:", err),
    );
  };

  return (
    <div className="flex flex-col gap-6">
      {/* Mic Mute toggle */}
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm font-medium text-white">Mic Mute</p>
          <p className="text-[11px] text-gray-500 mt-0.5">
            Silence the microphone
          </p>
        </div>
        <ToggleSwitch
          enabled={muted}
          onChange={handleMute}
          label="Mic Mute"
        />
      </div>

      {/* Read-only status indicator */}
      <div className="flex items-center gap-3 p-3 rounded-lg bg-white/5 border border-white/5">
        <span
          className={[
            "w-2.5 h-2.5 rounded-full flex-shrink-0 transition-colors",
            muted ? "bg-red-500 shadow-[0_0_6px_#ef4444]" : "bg-razer-green shadow-[0_0_6px_#44d62c]",
          ].join(" ")}
          aria-hidden="true"
        />
        <div>
          <p className="text-xs font-semibold text-white">
            {muted ? "Microphone muted" : "Microphone active"}
          </p>
          <p className="text-[10px] text-gray-500 mt-0.5">
            {muted
              ? "Your voice is not being transmitted"
              : "Your voice is being transmitted"}
          </p>
        </div>
      </div>
    </div>
  );
}
