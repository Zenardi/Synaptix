---
name: Bug Report
about: Report a hardware or software failure to be processed by the Synaptix AI pipeline.
title: '[BUG] '
labels: bug
assignees: ''
---

## The Issue
> **Context:** What is the exact failure? (e.g., "The DPI command returns false and does not change the mouse sensitivity.")

**Description:**
[Replace this with a clear, concise description of the bug]

## Ground Truth (Required)
To feed this bug into our automated pipeline, we need exact hardware and state data.

* **Device Model:** [e.g., Razer Cobra Pro]
* **USB Product ID (PID):** [e.g., `0x00B0`]
* **Connection Type:** [Wired / 2.4GHz Dongle / Bluetooth]

## Steps to Reproduce
1. [Step 1]
2. [Step 2]
3. [Step 3]

## Expected vs. Actual State
* **Expected:** [e.g., Mouse changes to 800 DPI]
* **Actual:** [e.g., D-Bus command returns `(false,)` and sensitivity remains unchanged]

## Daemon Logs (CRITICAL)
*Run `journalctl --user -u synaptix-daemon.service -n 50 --no-pager` and paste the output below.*
```text
[Paste your logs here. Do not truncate them.]