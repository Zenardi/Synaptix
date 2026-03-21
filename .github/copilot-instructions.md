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