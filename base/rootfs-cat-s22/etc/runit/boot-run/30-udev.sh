#!/bin/sh
# devtmpfs covers the nodes the boot needs; udevd is still what creates the
# by-id symlinks and runs firmware loading for the radios.
udevd=/lib/systemd/systemd-udevd
[ -x "$udevd" ] || udevd=/usr/lib/systemd/systemd-udevd
[ -x "$udevd" ] && "$udevd" --daemon || true
