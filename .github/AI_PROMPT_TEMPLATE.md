# Synaptix AI Prompting Standard

To maintain code quality and prevent AI agents (Copilot, Cursor, etc.) from hallucinating hardware protocols or breaking the Rust architecture, all feature requests fed to AI must follow the **Milestone Prompt Template**.

## The Philosophy
1. **Blast Radius Control:** Never ask the AI to "implement a feature" globally. Confine it to specific crates and files.
2. **Zero-Trust Math:** Never trust the AI to know Razer USB hex codes or magic numbers. Provide the exact mathematical constants or byte arrays in the prompt.
3. **Mandatory TDD:** The AI must write the test *before* wiring the routing logic. If the tests aren't green, the milestone fails.
4. **Hard Stops:** Explicitly tell the AI when to stop generating code to prevent it from rewriting unrelated files.

---

## The Prompt Template

Copy the blockquote below, fill in the bracketed variables, and paste it into your AI agent.

> **Milestone [NUMBER]: [FEATURE NAME]**
> 
> We are expanding the Synaptix architecture. We need to [ONE SENTENCE EXPLANATION OF THE GOAL AND WHY IT MATTERS].
> 
> **Your Task: [ACTIONABLE SUMMARY OF THE GOAL]**
> 
> **Step 1: The Data Contract (`[CRATE_NAME/FILE_PATH]`)**
> * Open `[EXACT_FILE_PATH]`.
> * Update the [ENUM/STRUCT/TRAIT] to include `[NEW_VARIANTS_OR_FIELDS]`.
> 
> **Step 2: Core Logic & Payload Math (`[CRATE_NAME/FILE_PATH]`)**
> * Open `[EXACT_FILE_PATH]`.
> * Create the function `[FUNCTION_SIGNATURE]`.
> * **CRITICAL RULES:** [INSERT ANY HARDCODED HEX VALUES, USB PIDS, OR MATH FORMULAS HERE. DO NOT LET THE AI GUESS].
> 
> **Step 3: TDD Verification**
> * Write a test `[TEST_NAME]` in the same file.
> * Assert that the function produces this exact expected output: `[INSERT EXPECTED BYTE ARRAY OR STATE HERE]`.
> 
> **Step 4: Device Routing & Wiring (`[CRATE_NAME/FILE_PATH]`)**
> * Open `[EXACT_FILE_PATH]`.
> * Update the [D-BUS METHOD / POLLING LOOP / ROUTER] to handle the new data contract. 
> * Connect it to the core logic built in Step 2.
> 
> **Step 5: Frontend Integration (If Applicable)**
> * Open `[REACT_COMPONENT_PATH]`.
> * Add the UI elements to trigger this state. Ensure the payload matches the expected JSON structure for the Tauri D-Bus bridge.
> 
> **Stop immediately after Step [X] is complete and the tests are green. Do not refactor unrelated code.**

---

## Example Usage

```md
> **Milestone 12: Application Branding and Icons**
> 
> We are replacing the default Tauri placeholder icons with our custom Synaptix branding so the `.deb` package installs with a native desktop icon.
> 
> **Your Task: Update the Tauri Icon Set.**
> 
> **Step 1: Locate the Source Image**
> * **[USER NOTE: Place your `1024x1024` PNG in the root of your project and name it `app-icon.png` before running this prompt]**
> * Verify that `app-icon.png` exists in the workspace root.
> 
> **Step 2: Run the Tauri Icon Generator**
> * Execute the Tauri icon generation CLI command: `npx tauri icon app-icon.png` (or `npm run tauri icon app-icon.png` depending on the package manager setup).
> * This will automatically overwrite the existing placeholder icons inside `crates/synaptix-ui/src-tauri/icons/`.
> 
> **Step 3: Verify the Build Configuration**
> * Open `crates/synaptix-ui/src-tauri/tauri.conf.json`.
> * Ensure the `bundle.icon` array is correctly pointing to the newly generated files in the `icons/` directory. (e.g., `"icons/32x32.png"`, `"icons/128x128.png"`, `"icons/128x128@2x.png"`, `"icons/icon.icns"`, `"icons/icon.ico"`).
> * No further code changes are needed. Stop and wait for review.
```


## Example to fix something

```markdown
> **Correction: D-Bus Session Mismatch (Systemd User Service Migration)**
> 
> The `.deb` package installs correctly, but the UI throws `org.freedesktop.DBus.Error.ServiceUnknown`. This is because the daemon is installed as a System service but attempts to bind to the Session D-Bus, resulting in IPC isolation from the UI. 
> 
> **Your Task: Migrate the daemon to a systemd User Service.**
> 
> **Step 1: Update the Systemd Unit File**
> * Open `packaging/synaptix-daemon.service`.
> * Change the `[Install]` section. Replace `WantedBy=multi-user.target` with `WantedBy=default.target`. (This is required for user services).
> 
> **Step 2: Update the Tauri Bundler Paths**
> * Open `crates/synaptix-ui/src-tauri/tauri.conf.json`.
> * In the `linux.deb.files` mapping, change the destination path for the service file.
> * Change `"/usr/lib/systemd/system/synaptix-daemon.service"` to `"/usr/lib/systemd/user/synaptix-daemon.service"`.
> 
> **Step 3: Update the Post-Install Script**
> * Open `packaging/post_install.sh`.
> * Remove the standard `systemctl enable --now` command.
> * Replace it with the global user service enabler: 
>   `systemctl --global enable synaptix-daemon.service`
> * *(Note: We cannot reliably use `--now` with `--global` during a root apt installation to start it for the local user, so the user will either reboot or start it manually the first time).*
> 
> Implement these changes so the daemon runs under the user's session and correctly attaches to the Session D-Bus.
***
``