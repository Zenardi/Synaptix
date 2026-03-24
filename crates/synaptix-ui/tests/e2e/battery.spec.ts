/**
 * battery.spec.ts — WebDriver E2E tests for the Synaptix battery display.
 *
 * These tests launch the REAL compiled Tauri binary via tauri-driver and verify
 * the rendered HTML using WebDriver.
 *
 * Prerequisites:
 *   - webkit2gtk-driver installed  (`sudo apt install webkit2gtk-driver`)
 *   - tauri-driver installed        (`cargo install tauri-driver --locked`)
 *   - app built                     (`cargo build --release` at workspace root)
 *
 * Run:
 *   npm run test:e2e
 *
 * The tests are written to work regardless of whether the synaptix-daemon is
 * running.  When the daemon is up, device cards are expected.  When it is down
 * the D-Bus call can take up to ~30 s to timeout before "Daemon unavailable" is
 * shown; the tests budget 45 s for this.
 */

/** Resolves to true if the "Daemon unavailable" banner is visible, false if
 * at least one device card appeared, or null if we're still waiting.
 *
 * Uses CSS class selectors instead of text-content selectors because
 * WebKitWebDriver text-content matching (*=text) can be unreliable.
 */
async function waitForAppReady(timeoutMs = 45_000): Promise<"error" | "devices" | "empty"> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    // Error banner: App.tsx renders a div.text-red-400 when invoke fails.
    const errorBanner = await $(".text-red-400");
    if (await errorBanner.isExisting()) return "error";

    // Device cards: DeviceCard root has .rounded-xl.
    const cards = await $$(".rounded-xl");
    if (cards.length > 0) return "devices";

    // "No devices connected." — App.tsx renders this as .text-gray-600.
    const empty = await $(".text-gray-600");
    if (await empty.isExisting()) return "empty";

    await browser.pause(500);
  }
  throw new Error("App did not reach a stable state within the timeout");
}

describe("Synaptix E2E — battery display", () => {
  let appState: "error" | "devices" | "empty";

  before(async () => {
    // Wait once for the app to settle; individual tests re-use the result.
    appState = await waitForAppReady(45_000);
  });

  it("app launches and shows the Synaptix heading", async () => {
    const heading = await $("h1");
    await expect(heading).toBeExisting();
    // Tailwind `uppercase` CSS causes WebKitWebDriver to return the
    // CSS-transformed text (all caps).
    await expect(heading).toHaveText("SYNAPTIX");
  });

  it("app reaches a stable state: daemon error, devices, or empty list", async () => {
    // This meta-test asserts the `before` hook settled correctly.
    expect(["error", "devices", "empty"]).toContain(appState);
  });

  it("shows 'Daemon unavailable' banner when daemon is not reachable", async () => {
    if (appState !== "error") {
      console.log(`[skip] Daemon is up (state=${appState}); skipping error-state test`);
      return;
    }
    // The error banner has class text-red-400 (defined in App.tsx).
    const banner = await $(".text-red-400");
    await expect(banner).toBeDisplayed();
    await expect(banner).toHaveTextContaining("Daemon unavailable");
  });

  it("device grid container is always present in the DOM", async () => {
    const grid = await $(".grid");
    await expect(grid).toBeExisting();
  });

  // ── Daemon-dependent tests ─────────────────────────────────────────────────
  // Only run when the app successfully retrieved at least one device.

  it("shows at least one DeviceCard when daemon is running", async () => {
    if (appState !== "devices") {
      console.log(`[skip] No devices available (state=${appState})`);
      return;
    }
    const cards = await $$(".rounded-xl");
    expect(cards.length).toBeGreaterThan(0);
  });

  it("battery ring SVG is present when a device card is shown", async () => {
    if (appState !== "devices") {
      console.log(`[skip] No devices available (state=${appState})`);
      return;
    }
    const ring = await $('[aria-label*="Battery"]');
    await expect(ring).toBeDisplayed();
  });
});
