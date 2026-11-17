#!/bin/sh
# Bring up the modem (MSS) so QRTR/QMI have something to talk to.
#
# The firmware lives on the phone's `modem` partition, not in the rootfs:
# mba.mbn plus the split modem.mdt/modem.bNN image. Stage them at the firmware
# root (the remoteproc driver asks for `mba.mbn` and `modem.mdt` by bare name),
# then start the mss remoteproc. Without this the modem stays offline: no QRTR,
# no QMI, no cellular data.
set -u

# udev does not run here, so /dev/disk/by-partlabel is empty. The kernel still
# exposes the GPT name per partition, which is the same information.
modem_part=
# The same boot race 40-wcnss.sh has: at stage 1 the partition nodes may not be
# there yet, and exiting quietly here means no modem for the rest of the boot.
i=0
while [ "$i" -lt 30 ]; do
    for uevent in /sys/class/block/mmcblk0p*/uevent; do
        grep -q '^PARTNAME=modem$' "$uevent" 2>/dev/null || continue
        block=$(basename "$(dirname "$uevent")")
        modem_part="/dev/$block"
        break
    done
    [ -n "$modem_part" ] && break
    sleep 1
    i=$((i + 1))
done
[ -n "$modem_part" ] || {
    echo "modem: no modem partition after ${i}s" > /dev/kmsg
    exit 0
}

mkdir -p /mnt/modem
mountpoint -q /mnt/modem || mount -o ro "$modem_part" /mnt/modem 2>/dev/null
[ -d /mnt/modem/image ] || exit 0

cp -f /mnt/modem/image/mba.mbn /lib/firmware/ 2>/dev/null
cp -f /mnt/modem/image/modem.mdt /mnt/modem/image/modem.b* /lib/firmware/ 2>/dev/null

for r in /sys/class/remoteproc/*/; do
    [ "$(cat "$r/name" 2>/dev/null)" = "4080000.remoteproc" ] || continue
    [ "$(cat "$r/state" 2>/dev/null)" = "offline" ] || continue
    echo start > "$r/state" 2>/dev/null
done

# Bind bam-dmux the moment the firmware is up, not whenever the deferred-probe
# list gets around to it. The modem asks the AP for a power collapse as part of
# its own initialization and waits for the acknowledgement; if this driver is
# not bound yet there is nobody to answer, and the modem's own watchdog fires
# ("dog.c: Watchdog detects stalled initialization") a minute or two later.
i=0
while [ "$i" -lt 40 ]; do
    if [ -e /sys/bus/platform/devices/4080000.remoteproc:bam-dmux ] &&
       [ ! -e /sys/bus/platform/devices/4080000.remoteproc:bam-dmux/driver ]; then
        echo 4080000.remoteproc:bam-dmux > /sys/bus/platform/drivers/bam-dmux/bind 2>/dev/null
    fi
    [ -e /sys/bus/platform/devices/4080000.remoteproc:bam-dmux/driver ] && break
    i=$((i + 1))
    sleep 0.5
done
