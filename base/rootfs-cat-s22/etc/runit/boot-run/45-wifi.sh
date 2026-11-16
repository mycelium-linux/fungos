#!/bin/sh
# Join the configured wireless network, if one is configured.
#
# 40-wcnss.sh has already started the firmware; this waits for the interface the
# driver creates, then hands it to wpa_supplicant and takes a lease.
set -u
conf=/etc/wpa_supplicant.conf
[ -f "$conf" ] || exit 0

i=0
while [ ! -e /sys/class/net/wlan0 ] && [ "$i" -lt 50 ]; do
    i=$((i + 1))
    sleep 0.1
done
[ -e /sys/class/net/wlan0 ] || exit 0

pkill -x wpa_supplicant 2>/dev/null
wpa_supplicant -B -i wlan0 -c "$conf" -D nl80211 >/dev/null 2>&1
dhcpcd -t 20 wlan0 >/dev/null 2>&1

# The radio sleeps between beacons by default: measured 14-96 ms round trips on
# a -50 dBm link, against 1-10 ms for the same AP from the Mac. wpa_supplicant
# re-enables power save when it associates, so turn it off after that, not
# before. This is what makes an SSH session over wlan0 usable.
iw dev wlan0 set power_save off 2>/dev/null
