/**
 * wdio.conf.ts — WebdriverIO configuration for Tauri E2E tests.
 *
 * Prerequisites (one-time setup):
 *   sudo apt install webkit2gtk-driver          # Linux WebKitWebDriver
 *   cargo install tauri-driver --locked          # Tauri-specific WD wrapper
 *
 * Run with:
 *   npm run test:e2e
 *
 * How it works:
 *   1. `onPrepare`: builds the app binary if it's missing, then starts tauri-driver.
 *   2. WebdriverIO connects to tauri-driver which launches the Tauri binary.
 *   3. Tests interact with the live app via WebDriver.
 *   4. `onComplete`: kills tauri-driver.
 *
 * Binary location: the Cargo workspace places the release binary at
 *   <workspace-root>/target/release/synaptix-ui
 * not inside src-tauri/target/ (which doesn't exist in a workspace setup).
 */

import { spawn, execSync, type ChildProcess } from "child_process";
import path from "path";
import fs from "fs";
import { fileURLToPath } from "url";
import type { Options } from "@wdio/types";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Cargo workspace root is two levels up from crates/synaptix-ui/.
const WORKSPACE_ROOT = path.resolve(__dirname, "..", "..");

// Release binary produced by `cargo tauri build` (placed in the workspace
// target directory, not inside src-tauri/).
const APP_BINARY = path.join(WORKSPACE_ROOT, "target", "release", "synaptix-ui");

let tauriDriver: ChildProcess | undefined;

export const config: Options.Testrunner = {
  runner: "local",

  specs: ["tests/e2e/**/*.spec.ts"],
  exclude: [],

  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      // "wry" is the WebKitGTK WebDriver browser name used by tauri-driver on Linux.
      browserName: "wry",
      "tauri:options": {
        application: APP_BINARY,
      },
      "wdio:enforceWebDriverClassic": true,
    },
  ],

  logLevel: "info",
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  // tauri-driver listens on 4444 by default.
  hostname: "localhost",
  port: 4444,
  path: "/",

  framework: "mocha",
  reporters: ["spec"],

  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  // ── Lifecycle hooks ──────────────────────────────────────────────────────────

  onPrepare(): Promise<void> {
    // Build the release binary if it doesn't exist yet.
    if (!fs.existsSync(APP_BINARY)) {
      console.log(`[E2E] Binary not found at ${APP_BINARY}`);
      console.log("[E2E] Running: cargo build --release (this may take a few minutes)…");
      try {
        execSync("cargo build --release", {
          cwd: WORKSPACE_ROOT,
          stdio: "inherit",
        });
      } catch (err) {
        throw new Error(
          `[E2E] cargo build --release failed.\n` +
          `Make sure the Rust toolchain is installed and run:\n` +
          `  cd ${WORKSPACE_ROOT} && cargo build --release`,
        );
      }
    }

    console.log(`[E2E] Using binary: ${APP_BINARY}`);

    return new Promise((resolve, reject) => {
      tauriDriver = spawn("tauri-driver", [], {
        stdio: [null, process.stdout, process.stderr],
      });
      tauriDriver.on("error", (err) => {
        reject(
          new Error(
            `[E2E] Failed to start tauri-driver: ${err.message}\n` +
              "Install with: cargo install tauri-driver --locked",
          ),
        );
      });
      // Give tauri-driver 2 seconds to start up before WebdriverIO connects.
      setTimeout(resolve, 2000);
    });
  },

  onComplete(): void {
    tauriDriver?.kill();
  },
};
