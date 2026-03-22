---
name: Feature Request
about: Propose a new hardware feature or device integration.
title: '[FEATURE] '
labels: enhancement
assignees: ''
---

## The Goal
> **Context:** What are we adding to the Synaptix ecosystem? (e.g., "Add support for the 'Breathing' lighting effect on the BlackWidow V3.")

**Description:**
[Replace this with a detailed explanation of the feature and why it matters.]

## Hardware Capabilities
* **Target Device(s):** [e.g., All keyboards, or specifically the Basilisk V3]
* **Target PID(s):** [e.g., `0x024E`]

## Legacy Reference (Required for Hardware Features)
To help our AI agent map the raw USB payload math, please locate the feature in the legacy Python codebase.
* **OpenRazer Python File:** [e.g., `daemon/openrazer_daemon/hardware/keyboard.py`]
* **Known Command Class/ID:** [e.g., Class `0x0F`, ID `0x03`] (If known, otherwise leave blank)

## Expected Architecture Changes
* **UI/Frontend:** [How should this look in the React app? e.g., "Add a dropdown for breathing effects next to the color picker."]
* **Daemon/Backend:** [What D-Bus method needs to be added? e.g., "Add a `SetBreathing` method to the D-Bus router."]

---

> [!NOTE]
> Maintainer: To implement this, copy this request into the `Milestone Prompt Template`, define the specific data contracts in `synaptix-protocol`, and execute via Copilot using TDD constraints.