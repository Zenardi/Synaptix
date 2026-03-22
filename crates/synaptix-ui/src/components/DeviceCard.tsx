import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import type { RazerDevice, ConnectionType } from "../App";
import { getBatteryLevel, isCharging } from "../App";

const RADIUS = 45;
const STROKE_WIDTH = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

const CONNECTION_META: Record<ConnectionType, { icon: string; label: string; color: string }> = {
  Wired:     { icon: "⚡", label: "Wired",      color: "text-[#44d62c]" },
  Dongle:    { icon: "📡", label: "USB Dongle", color: "text-blue-400"  },
  Bluetooth: { icon: "🔵", label: "Bluetooth",  color: "text-sky-400"   },
};

interface Props {
  device: RazerDevice;
}

export default function DeviceCard({ device }: Props) {
  const navigate = useNavigate();
  const level = getBatteryLevel(device.battery_state);
  const charging = isCharging(device.battery_state, device.connection_type);
  const targetOffset = CIRCUMFERENCE * (1 - level / 100);

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
            animate={{
              strokeDashoffset: targetOffset,
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
