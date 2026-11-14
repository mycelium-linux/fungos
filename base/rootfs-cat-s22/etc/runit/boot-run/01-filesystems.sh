#!/bin/sh
# switch_root carries the initramfs mounts over, but the userland still needs
# /proc early (mycelium and udev both fail without it) and a clean /run: a
# stale /run/dbus/pid from the initramfs makes dbus refuse to start.
mountpoint -q /proc 2>/dev/null || mount -t proc proc /proc
mountpoint -q /sys 2>/dev/null || mount -t sysfs sysfs /sys
mountpoint -q /dev 2>/dev/null || mount -t devtmpfs devtmpfs /dev
mountpoint -q /run 2>/dev/null || mount -t tmpfs -o mode=0755 tmpfs /run
mkdir -p /run/dbus
rm -f /run/dbus/pid
