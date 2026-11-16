#!/bin/sh
# Serve the modem's EFS before 41-modem.sh starts the firmware.
#
# The modem reads modemst1/modemst2/fsg/fsc through rmtfs over QRTR while it
# initializes. Without it the firmware watchdog-fires on every boot
# ("dog.c: Watchdog detects stalled initialization", every ~40 s), never
# completes init and never publishes the data channels.
#
# The stock partitions are only ever read: each is copied once into /boot
# (which lives on userdata) and rmtfs is pointed at the copies through
# /dev/disk/by-partlabel. The modem writes to its EFS as part of normal
# operation, and those writes must not land on eMMC.
set -u

# udev does not run here, so by-partlabel is empty; the kernel still exposes the
# GPT name per partition, which is the same information.
block_for() {
    for uevent in /sys/class/block/mmcblk0p*/uevent; do
        grep -q "^PARTNAME=$1\$" "$uevent" 2>/dev/null || continue
        basename "$(dirname "$uevent")"
        return 0
    done
    return 1
}

stage() {
    [ -e "$2" ] && return 0
    block=$(block_for "$1") || return 0
    cp -f "/dev/$block" "$2"
}

stage modemst1 /boot/modem_fs1
stage modemst2 /boot/modem_fs2
stage fsg      /boot/modem_fsg
stage fsc      /boot/modem_fsc

mkdir -p /dev/disk/by-partlabel
ln -sfn /boot/modem_fs1 /dev/disk/by-partlabel/modemst1
ln -sfn /boot/modem_fs2 /dev/disk/by-partlabel/modemst2
ln -sfn /boot/modem_fsg /dev/disk/by-partlabel/fsg
ln -sfn /boot/modem_fsc /dev/disk/by-partlabel/fsc

[ -e /boot/modem_fs1 ] || exit 0

mkdir -p /var/log
pkill -x rmtfs 2>/dev/null
setsid rmtfs -v >/var/log/rmtfs.log 2>&1 </dev/null &
