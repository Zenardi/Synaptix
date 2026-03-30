interface Props {
  onRetry: () => void;
}

const COMMANDS = [
  "systemctl --user daemon-reload",
  "systemctl --user start synaptix-daemon.service",
] as const;

function CommandBlock({ cmd }: { cmd: string }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2 px-3 bg-[#0d0d0d] rounded border border-white/10 font-mono text-sm text-razer-green">
      <span>{cmd}</span>
      <button
        onClick={() => navigator.clipboard.writeText(cmd)}
        title="Copy to clipboard"
        className="shrink-0 text-gray-500 hover:text-razer-green transition-colors text-xs"
      >
        copy
      </button>
    </div>
  );
}

export default function DaemonUnavailableScreen({ onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center min-h-[60vh] text-center px-8">
      {/* Warning icon */}
      <svg
        className="w-12 h-12 text-razer-green mb-5 opacity-80"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={1.5}
        aria-hidden="true"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"
        />
      </svg>

      <h2 className="text-xl font-bold text-white mb-3 tracking-wide">
        Synaptix Daemon is not running
      </h2>

      <p className="text-gray-400 text-sm mb-6 max-w-md leading-relaxed">
        If this is your first time installing Synaptix, you may need to either{" "}
        <span className="text-white font-medium">reboot your machine</span> or
        manually start the service for this session by running:
      </p>

      <div className="flex flex-col gap-2 w-full max-w-md mb-8">
        {COMMANDS.map((cmd) => (
          <CommandBlock key={cmd} cmd={cmd} />
        ))}
      </div>

      <button
        onClick={onRetry}
        className="px-6 py-2 bg-razer-green/10 border border-razer-green text-razer-green text-sm font-medium rounded-md hover:bg-razer-green/20 transition-colors"
      >
        Retry Connection
      </button>
    </div>
  );
}
