#!/bin/sh
# The initramfs offers a root shell on the USB serial gadget; keep that when the
# userland takes over. agetty depends on a mounted /proc and on the login
# program working, and when either is missing the only console on the phone is
# silent.
[ -c /dev/ttyGS0 ] || exit 0
setsid sh -c 'exec sh -i </dev/ttyGS0 >/dev/ttyGS0 2>&1' &
