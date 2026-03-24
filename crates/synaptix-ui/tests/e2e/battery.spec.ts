/**
 * battery.spec.ts — WebDriver E2E tests for the Synaptix battery display.
 *
 * These tests launch the REAL compiled Tauri binary via tauri-driver and verify
 * the rendered HTML using WebDriver.
 *
 * Prerequisites:
 *   - webkit2gtk-driver installed  (`sudo apt install webkit2gtk-driver`)
 *   - tauri-driver installed        (`cargo install tauri-driver --locked`)
 *   - app built                     (`npm run tauri build`)
 *   - synaptix-daemon running       (optional; some tests work without it)
 *
 * Run:
 *   npm run test:e2e
 */

describe("Synaptix E2E — battery display", () => {
  it("app launches and shows the Synaptix heading", async () => {
    // The heading is rendered unconditionally, daemon not required.
    const heading = await $("h1");
    await expect(heading).toBeExisting();
    await expect(heading).toHaveText("Synaptix");
  });

  it("shows 'Daemon unavailable' when the daemon is not running", async () => {
    // If no daemon is reachable, App.tsx sets error and renders this message.
    // Allow up to 10 s for the IPC call to fail.
    const errorEl = await $("*=Daemon unavailable");
    await errorEl.waitForExist({ timeout: 10_000 });
    await expect(errorEl).toBeDisplayed();
  });

  it("device list area is present in the DOM", async () => {
    // The grid div is rendered unconditionally even when empty.
    const grid = await $(".grid");
    await expect(grid).toBeExisting();
  });

  // ── Daemon-dependent tests ─────────────────────────────────────────────────
  // These require the synaptix-daemon to be running.  They are skipped when
  // the daemon is unavailable so CI doesn't fail on headless build agents.

  it("shows at least one DeviceCard when daemon is running", async () => {
    const errorBanner = await $("*=Daemon unavailable");
    const daemonUp = !(await errorBanner.isExisting());
    if (!daemonUp) {
      console.log("Skipping: daemon not running");
      return;
    }

    // Wait for the device cards to appear (invoke resolves asynchronously).
    await browser.waitUntil(
      async () => {
        const cards = await $$(".rounded-xl");
        return cards.length > 0;
      },
      { timeout: 10_000, timeoutMsg: "No device cards appeared" },
    );

    const cards = await $$(".rounded-xl");
    expect(cards.length).toBeGreaterThan(0);
  });

  it("battery ring SVG is present when a device card is shown", async () => {
    const errorBanner = await $("*=Daemon unavailable");
    const daemonUp = !(await errorBanner.isExisting());
    if (!daemonUp) {
      console.log("Skipping: daemon not running");
      return;
    }

    await browser.waitUntil(
      async () => {
        const rings = await $$('[aria-label*="Battery"]');
        return rings.length > 0;
      },
      { timeout: 10_000, timeoutMsg: "No battery ring found" },
    );

    const ring = await $('[aria-label*="Battery"]');
    await expect(ring).toBeDisplayed();
  });
});
