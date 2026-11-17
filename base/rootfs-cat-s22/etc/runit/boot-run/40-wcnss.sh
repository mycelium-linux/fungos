#!/bin/sh
# Bring up the WCNSS (WiFi) subsystem.
#
# The firmware lives on the phone's own partitions, not in the rootfs: the
# remoteproc image in `modem` and the calibration NV in `persist`. Copy them
# where the driver looks, then start the remoteproc -- wcn36xx is built into the
# kernel and binds the `wifi` child node once the firmware is running.
set -u
# udev does not run here, so /dev/disk/by-partlabel is empty even though the GPT
# has the names. The kernel exposes the same name per partition.
partition() {
    for uevent in /sys/class/block/mmcblk0p*/uevent; do
        grep -q "^PARTNAME=$1\$" "$uevent" 2>/dev/null || continue
        echo "/dev/$(basename "$(dirname "$uevent")")"
        return 0
    done
    return 1
}
# Stage 1 runs before udev and, on a cold boot, before the mmc partitions are
# necessarily enumerated: the partlabel lookup then finds nothing and this
# script used to continue with no firmware, leaving WCNSS offline and the phone
# with no WiFi at all. Wait for the block devices rather than assume them.
i=0
while [ "$i" -lt 30 ] && ! partition modem >/dev/null; do
    sleep 1
    i=$((i + 1))
done

for part in modem persist; do
    mkdir -p "/mnt/$part"
    mountpoint -q "/mnt/$part" && continue
    device=$(partition "$part") || {
        echo "wcnss: no $part partition after ${i}s; firmware not staged" > /dev/kmsg
        continue
    }
    mount -o ro "$device" "/mnt/$part" 2>/dev/null
done

if [ -d /mnt/modem/image ]; then
    cp -f /mnt/modem/image/wcnss.* /usr/lib/firmware/ 2>/dev/null
fi
if [ -f /mnt/persist/WCNSS_qcom_wlan_nv.bin ]; then
    mkdir -p /usr/lib/firmware/wlan/prima
    cp -f /mnt/persist/WCNSS_qcom_wlan_nv.bin \
          /mnt/persist/WCNSS_wlan_dictionary.dat \
          /usr/lib/firmware/wlan/prima/ 2>/dev/null
fi

# Start every offline remoteproc that has firmware: WCNSS is the one that
# matters here, and starting it is what makes wcn36xx probe.
for r in /sys/class/remoteproc/*/; do
    [ "$(cat "$r/name" 2>/dev/null)" = "a204000.remoteproc" ] || continue
    [ "$(cat "$r/state" 2>/dev/null)" = "offline" ] || continue
    echo start > "$r/state" 2>/dev/null
done
