#!/usr/bin/env bash
set -euo pipefail

# 1. Apply the new udev rule so Razer devices are accessible without root.
udevadm control --reload-rules
udevadm trigger

# 2. Pick up the new systemd unit file.
systemctl daemon-reload

# 3. Enable and immediately start the daemon.
systemctl enable --now synaptix-daemon.service

echo "Synaptix daemon installed and started successfully."
echo "Add your user to the 'plugdev' group to control devices without sudo:"
echo "  sudo usermod -aG plugdev \$USER  (then log out and back in)"
