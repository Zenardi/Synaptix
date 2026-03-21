import { motion } from "framer-motion";
import type { RazerDevice } from "../App";
import { getBatteryLevel, isCharging } from "../App";

const RADIUS = 45;
const STROKE_WIDTH = 7;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

interface Props {
  device: RazerDevice;
}

export default function DeviceCard({ device }: Props) {
  const level = getBatteryLevel(device.battery_state);
  const charging = isCharging(device.battery_state);

  // stroke-dashoffset = 0 means fully drawn; CIRCUMFERENCE means fully hidden.
  const targetOffset = CIRCUMFERENCE * (1 - level / 100);

  return (
    <div className="bg-[#181818] rounded-xl p-6 border border-white/5 flex flex-col items-center gap-5">
      {/* Circular battery ring */}
      <div className="relative flex items-center justify-center">
        <svg
          className="-rotate-90"
          viewBox="0 0 100 100"
          width={128}
          height={128}
          aria-label={`Battery level ${level}%`}
        >
          {/* Background track */}
          <circle
            cx="50"
            cy="50"
            r={RADIUS}
            fill="none"
            stroke="#2a2a2a"
            strokeWidth={STROKE_WIDTH}
          />
          {/* Animated foreground arc */}
          <motion.circle
            cx="50"
            cy="50"
            r={RADIUS}
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

        {/* Centre percentage label (un-rotated) */}
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

      {/* Device name */}
      <p className="text-sm text-gray-300 font-medium text-center leading-snug">
        {device.name}
      </p>
    </div>
  );
}
