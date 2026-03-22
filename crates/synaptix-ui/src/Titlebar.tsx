import { getCurrentWindow } from "@tauri-apps/api/window";

export default function Titlebar() {
  return (
    <div
      data-tauri-drag-region
      className="fixed top-0 left-0 right-0 z-50 h-9 flex items-center justify-between px-4 bg-[#0d0d0d] border-b border-white/5 select-none"
    >
      {/* Wordmark — also part of the drag region */}
      <span
        data-tauri-drag-region
        className="text-xs font-bold tracking-[0.2em] uppercase text-razer-green pointer-events-none"
      >
        Synaptix
      </span>

      {/* Window controls — must NOT propagate drag, so no data-tauri-drag-region here */}
      <div className="flex items-center">
        {/* Minimize */}
        <button
          onClick={() => getCurrentWindow().minimize()}
          className="w-9 h-9 flex items-center justify-center text-gray-400 hover:bg-gray-800 hover:text-white transition-colors"
          aria-label="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1" fill="currentColor">
            <rect width="10" height="1" />
          </svg>
        </button>

        {/* Close */}
        <button
          onClick={() => getCurrentWindow().close()}
          className="w-9 h-9 flex items-center justify-center text-gray-400 hover:bg-red-500 hover:text-white transition-colors"
          aria-label="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor">
            <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      </div>
    </div>
  );
}
