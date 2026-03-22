#!/usr/bin/env bash
set -euo pipefail

# 1. Apply the new udev rule so Razer devices are accessible without root.
udevadm control --reload-rules
udevadm trigger

# 2. Pick up the new systemd user unit file.
systemctl daemon-reload

# 3. Enable the daemon globally for all user sessions.
#    --global writes the enable symlink into /etc/systemd/user/ so the unit
#    starts automatically when any user's session comes up.
#    We cannot use --now here because apt runs as root and the target user
#    session is not available at install time — the user should reboot or run:
#      systemctl --user start synaptix-daemon.service
systemctl --global enable synaptix-daemon.service

echo "Synaptix daemon installed successfully."
echo "It will start automatically on your next login."
echo "To start it immediately (without rebooting), run:"
echo "  systemctl --user start synaptix-daemon.service"
echo ""
echo "Add your user to the 'plugdev' group to control devices without sudo:"
echo "  sudo usermod -aG plugdev \$USER  (then log out and back in)"
