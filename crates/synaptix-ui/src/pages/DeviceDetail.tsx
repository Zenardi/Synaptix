import { useContext, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { DevicesContext, getBatteryLevel, isCharging, hasCapability } from "../App";
import type { RazerDevice } from "../App";
import AudioTab from "../components/headset/AudioTab";
import MicTab from "../components/headset/MicTab";
import HapticsTab from "../components/headset/HapticsTab";
import DpiControl from "../components/DpiControl";
import LightingControl from "../components/LightingControl";

// ── Types ─────────────────────────────────────────────────────────────────────

type TabId = "audio" | "mic" | "haptics" | "lighting" | "performance";

interface Tab {
  id: TabId;
  label: string;
  icon: string;
}

// ── Battery ring (compact version for the detail header) ─────────────────────

const RADIUS = 32;
const STROKE_WIDTH = 5;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function BatteryRing({ device }: { device: RazerDevice }) {
  const isUnknown = device.battery_state === "Unknown";
  const level = getBatteryLevel(device.battery_state);
  const charging = isCharging(device.battery_state, device.connection_type);
  const offset = isUnknown ? CIRCUMFERENCE : CIRCUMFERENCE * (1 - level / 100);

  return (
    <div className="relative flex items-center justify-center">
      <svg
        className="-rotate-90"
        viewBox="0 0 70 70"
        width={80}
        height={80}
        aria-label={isUnknown ? "Battery level unknown" : `Battery ${level}%`}
      >
        <circle cx="35" cy="35" r={RADIUS} fill="none" stroke="#2a2a2a" strokeWidth={STROKE_WIDTH} />
        <motion.circle
          cx="35" cy="35" r={RADIUS}
          fill="none"
          stroke={isUnknown ? "#4a4a4a" : "#44d62c"}
          strokeWidth={STROKE_WIDTH}
          strokeLinecap="round"
          strokeDasharray={CIRCUMFERENCE}
          initial={{ strokeDashoffset: CIRCUMFERENCE }}
          animate={{
            strokeDashoffset: offset,
            filter: charging
              ? ["drop-shadow(0 0 3px #44d62c)", "drop-shadow(0 0 8px #44d62c)", "drop-shadow(0 0 3px #44d62c)"]
              : isUnknown
              ? "none"
              : "drop-shadow(0 0 4px #44d62c)",
          }}
          transition={
            charging
              ? { strokeDashoffset: { duration: 1, ease: "easeOut" }, filter: { duration: 1.8, repeat: Infinity } }
              : { duration: 1, ease: "easeOut" }
          }
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center">
        <span className="text-sm font-bold text-white leading-none">
          {isUnknown ? "?" : `${level}%`}
        </span>
        {charging && (
          <span className="text-[8px] font-semibold uppercase tracking-wider text-razer-green">⚡</span>
        )}
      </div>
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export default function DeviceDetail() {
  const { deviceId } = useParams<{ deviceId: string }>();
  const navigate = useNavigate();
  const { devices } = useContext(DevicesContext);

  const device = devices.find((d) => d.device_id === deviceId);

  // ── Build the tab list based on device capabilities ───────────────────────
  const tabs: Tab[] = [];
  if (device) {
    if (hasCapability(device, "ThxSpatialAudio") || hasCapability(device, "Sidetone"))
      tabs.push({ id: "audio", label: "Audio", icon: "🎧" });
    if (hasCapability(device, "Microphone"))
      tabs.push({ id: "mic", label: "Mic", icon: "🎙️" });
    if (hasCapability(device, "HapticFeedback"))
      tabs.push({ id: "haptics", label: "Haptics", icon: "📳" });
    if (device.capabilities.some((c) => typeof c === "object" && "Lighting" in c) ||
        device.capabilities.some((c) => typeof c === "string" && c === "Lighting"))
      tabs.push({ id: "lighting", label: "Lighting", icon: "💡" });
    if (hasCapability(device, "DpiControl"))
      tabs.push({ id: "performance", label: "Performance", icon: "🎯" });
    // Fallback: always show at least lighting
    if (tabs.length === 0)
      tabs.push({ id: "lighting", label: "Lighting", icon: "💡" });
  }

  const [activeTab, setActiveTab] = useState<TabId>(tabs[0]?.id ?? "lighting");

  // ── Not found ─────────────────────────────────────────────────────────────
  if (!device) {
    return (
      <div className="pt-9 p-8 flex flex-col items-center justify-center gap-4 text-center">
        <p className="text-gray-500 text-sm">Device not found.</p>
        <button
          onClick={() => navigate("/")}
          className="text-razer-green text-sm hover:underline"
        >
          ← Back to Dashboard
        </button>
      </div>
    );
  }

  return (
    <div className="pt-9 min-h-screen">
      {/* ── Header ─────────────────────────────────────────────────────────── */}
      <div className="flex items-center gap-4 px-6 py-4 border-b border-white/5">
        <button
          onClick={() => navigate("/")}
          className="text-[11px] font-semibold tracking-widest uppercase text-gray-400 hover:text-razer-green transition-colors flex items-center gap-1"
        >
          ← Back
        </button>

        <div className="flex-1 flex items-center gap-4 min-w-0">
          {hasCapability(device, "BatteryReporting") && <BatteryRing device={device} />}
          <div className="min-w-0">
            <h1 className="text-lg font-bold text-white truncate leading-tight">
              {device.name}
            </h1>
            <p className="text-[10px] text-gray-500 tracking-widest uppercase mt-0.5">
              {device.connection_type}
            </p>
          </div>
        </div>
      </div>

      {/* ── Tab navigation ─────────────────────────────────────────────────── */}
      <div className="flex gap-1 px-6 py-3 border-b border-white/5 overflow-x-auto scrollbar-none">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={[
              "flex items-center gap-1.5 px-4 py-2 rounded-full text-[11px] font-semibold tracking-widest uppercase whitespace-nowrap transition-all",
              activeTab === tab.id
                ? "bg-razer-green text-black shadow-[0_0_10px_#44d62c55]"
                : "bg-white/5 text-gray-400 hover:bg-white/10 hover:text-white",
            ].join(" ")}
          >
            <span aria-hidden="true">{tab.icon}</span>
            {tab.label}
          </button>
        ))}
      </div>

      {/* ── Tab content ────────────────────────────────────────────────────── */}
      <div className="p-6">
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.15 }}
          >
            {activeTab === "audio" && (
              <AudioTab deviceId={device.device_id} pid={String(deviceId)} />
            )}
            {activeTab === "mic" && (
              <MicTab deviceId={device.device_id} pid={String(deviceId)} />
            )}
            {activeTab === "haptics" && (
              <HapticsTab deviceId={device.device_id} pid={String(deviceId)} />
            )}
            {activeTab === "lighting" && (
              <LightingControl deviceId={device.device_id} />
            )}
            {activeTab === "performance" && (
              <div className="max-w-xs">
                <DpiControl deviceId={device.device_id} />
              </div>
            )}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
