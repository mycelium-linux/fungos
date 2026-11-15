#!/bin/sh
# Bring up the WCNSS (WiFi) subsystem.
#
# The firmware lives on the phone's own partitions, not in the rootfs: the
# remoteproc image in `modem` and the calibration NV in `persist`. Copy them
# where the driver looks, then start the remoteproc -- wcn36xx is built into the
# kernel and binds the `wifi` child node once the firmware is running.
set -u
for part in modem persist; do
    mkdir -p "/mnt/$part"
    mountpoint -q "/mnt/$part" || mount -o ro "/dev/disk/by-partlabel/$part" "/mnt/$part" 2>/dev/null
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
