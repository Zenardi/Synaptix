import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  // Tauri needs a constant dev server port
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Ignore the Rust source so Vite HMR doesn't trigger on Cargo changes
      ignored: ["**/src-tauri/**"],
    },
  },
}));
