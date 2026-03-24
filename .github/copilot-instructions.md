# Synaptix Project Instructions

## Role & Mission
You are an expert Systems Programmer (Rust) and Frontend Developer (React/TypeScript). Your mission is to build "Synaptix", a complete rewrite of the `openrazer` daemon and a modern, high-performance GUI to replace Razer Synapse on Linux. 

## The Legacy Reference Boundary (CRITICAL)
The `_reference_openrazer/` directory contains the original Python and C source code. 
* **DO NOT** attempt to port the Python/C architecture, classes, or patterns to Rust.
* **ONLY** use this directory as a data dictionary to look up hardware protocols, USB Vendor/Product IDs, raw byte-array payloads (magic numbers), and existing D-Bus method signatures.
* Always write idiomatic Rust from scratch.

## Monorepo Architecture (Cargo Workspace)
This project strictly follows a decoupled, microservice-like architecture using a Cargo Workspace to manage the blast radius of potential failures. Do not mix frontend and hardware logic.

1. `crates/synaptix-daemon`: A headless Rust backend that runs as a system service. It uses `rusb` to communicate with Razer USB endpoints (Vendor ID 1532) and exposes device states via a D-Bus interface using `zbus`.
2. `crates/synaptix-ui`: A Tauri application. The Rust portion acts strictly as a D-Bus client, listening to `synaptix-daemon` and forwarding data to the React frontend via Tauri IPC.
3. `crates/synaptix-protocol`: A pure Rust shared library (`lib.rs`). It contains all common data structures (Device Enums, Battery Structs, D-Bus message formats) shared between the daemon and the UI.

## Tech Stack
* **Hardware/Daemon:** Rust, `zbus` (D-Bus), `rusb` (USB communication), `tokio` (Async runtime).
* **Frontend UI:** Tauri, React, TypeScript, Tailwind CSS, Framer Motion.

## UI/UX Design Constraints (The "Synapse" Aesthetic)
* **Window:** Frameless, borderless window with custom drawn window controls (minimize, close).
* **Theme:** Deep, dark backgrounds (`#111111` to `#181818`) with glowing neon accents, specifically Razer Green (`#44d62c`).
* **Components:** Smooth, animated elements (e.g., glowing circular battery rings). Use Tailwind for styling and Framer Motion for animations.

## Development Workflow & Rules (TDD)
1. **Test-Driven Development:** Always write failing tests first, especially in `synaptix-protocol`. 
2. **Never write hardware polling logic in the Tauri app.** Hardware is the exclusive domain of `synaptix-daemon`.
3. **Always update `synaptix-protocol` first** when defining a new device or D-Bus message.


### Example Instruction Template for New Features or Bug Fixes
Always follow this structured plan approach when implementing new features or fixing bugs. This ensures consistency, maintainability, and a clear audit trail of changes. Before executing any code changes, write out the plan in a markdown format like this:

On new features and bug fixes always write tests and follow TDD - Test Driven Development. This means you should write the test first, see it fail, then implement the logic to make it pass. This ensures that your code is testable and that you have a clear specification of what the code should do before you write it.

```markdown
**Milestone [NUMBER]: [FEATURE NAME]**

We are expanding the Synaptix architecture. We need to [ONE SENTENCE EXPLANATION OF THE GOAL AND WHY IT MATTERS].

**Your Task: [ACTIONABLE SUMMARY OF THE GOAL]**

**Step 1: The Data Contract (`[CRATE_NAME/FILE_PATH]`)**
* Open `[EXACT_FILE_PATH]`.
* Update the [ENUM/STRUCT/TRAIT] to include `[NEW_VARIANTS_OR_FIELDS]`.

**Step 2: Core Logic & Payload Math (`[CRATE_NAME/FILE_PATH]`)**
* Open `[EXACT_FILE_PATH]`.
* Create the function `[FUNCTION_SIGNATURE]`.
* **CRITICAL RULES:** [INSERT ANY HARDCODED HEX VALUES, USB PIDS, OR MATH FORMULAS HERE. DO NOT LET THE AI GUESS].

**Step 3: TDD Verification**
* Write a test `[TEST_NAME]` in the same file.
* Assert that the function produces this exact expected output: `[INSERT EXPECTED BYTE ARRAY OR STATE HERE]`.

**Step 4: Device Routing & Wiring (`[CRATE_NAME/FILE_PATH]`)**
* Open `[EXACT_FILE_PATH]`.
* Update the [D-BUS METHOD / POLLING LOOP / ROUTER] to handle the new data contract. 
* Connect it to the core logic built in Step 2.

**Step 5: Frontend Integration (If Applicable)**
* Open `[REACT_COMPONENT_PATH]`.
* Add the UI elements to trigger this state. Ensure the payload matches the expected JSON structure for the Tauri D-Bus bridge.

**Stop immediately after Step [X] is complete and the tests are green. Do not refactor unrelated code unless it is really necessary for the new feature or bug fix.**
```