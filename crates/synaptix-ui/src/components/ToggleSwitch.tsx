import { motion } from "framer-motion";

interface Props {
  enabled: boolean;
  onChange: (next: boolean) => void;
  label?: string;
  disabled?: boolean;
}

/**
 * iOS-style toggle switch. Glows Razer Green (#44d62c) when active.
 */
export default function ToggleSwitch({
  enabled,
  onChange,
  label,
  disabled = false,
}: Props) {
  return (
    <button
      role="switch"
      aria-checked={enabled}
      aria-label={label}
      disabled={disabled}
      onClick={() => !disabled && onChange(!enabled)}
      className={[
        "relative inline-flex items-center w-11 h-6 rounded-full transition-all duration-300 focus:outline-none",
        enabled
          ? "bg-razer-green shadow-[0_0_10px_#44d62c55]"
          : "bg-white/10",
        disabled ? "opacity-40 cursor-not-allowed" : "cursor-pointer",
      ].join(" ")}
    >
      <motion.span
        layout
        transition={{ type: "spring", stiffness: 500, damping: 35 }}
        className={[
          "inline-block w-4 h-4 rounded-full bg-white shadow-md",
          enabled ? "translate-x-6" : "translate-x-1",
        ].join(" ")}
      />
    </button>
  );
}
