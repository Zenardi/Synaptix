import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    // E2E specs require a live Tauri binary and tauri-driver; they are run
    // separately via `npm run test:e2e` (WebdriverIO), not by Vitest.
    exclude: ["**/node_modules/**", "tests/e2e/**"],
  },
});
