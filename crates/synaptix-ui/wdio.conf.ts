/**
 * wdio.conf.ts — WebdriverIO configuration for Tauri E2E tests.
 *
 * Prerequisites (one-time setup):
 *   sudo apt install webkit2gtk-driver          # Linux WebKitWebDriver
 *   cargo install tauri-driver --locked          # Tauri-specific WD wrapper
 *   npm run tauri build                          # Build the release binary
 *
 * Run with:
 *   npm run test:e2e
 *
 * How it works:
 *   1. `onPrepare`: starts `tauri-driver` on port 4444.
 *   2. WebdriverIO connects to tauri-driver which launches the Tauri binary.
 *   3. Tests interact with the live app via WebDriver.
 *   4. `onComplete`: kills tauri-driver.
 */

import { spawn, type ChildProcess } from "child_process";
import path from "path";
import { fileURLToPath } from "url";
import type { Options } from "@wdio/types";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Path to the release binary produced by `npm run tauri build`.
const APP_BINARY = path.resolve(
  __dirname,
  "src-tauri",
  "target",
  "release",
  "synaptix-ui",
);

let tauriDriver: ChildProcess | undefined;

export const config: Options.Testrunner = {
  runner: "local",

  specs: ["tests/e2e/**/*.spec.ts"],
  exclude: [],

  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      browserName: "",
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
    return new Promise((resolve, reject) => {
      tauriDriver = spawn("tauri-driver", [], {
        stdio: [null, process.stdout, process.stderr],
      });
      tauriDriver.on("error", (err) => {
        reject(
          new Error(
            `Failed to start tauri-driver: ${err.message}\n` +
              "Install with: cargo install tauri-driver --locked",
          ),
        );
      });
      // Give tauri-driver 2 seconds to start up.
      setTimeout(resolve, 2000);
    });
  },

  onComplete(): void {
    tauriDriver?.kill();
  },
};
