#!/bin/sh
# The initramfs mounts userdata before switch_root; that mount does not survive,
# so the userland owns it from here. Without it /data is an empty directory and
# every log and state file underneath it silently lives in RAM.
mkdir -p /mnt/userdata
mountpoint -q /mnt/userdata 2>/dev/null || \
	mount -t f2fs /dev/mmcblk0p56 /mnt/userdata || echo "userdata: mount failed"
mkdir -p /mnt/userdata/cat-s22-data
# If the image already has a /data directory, ln -sfn would create the link
# inside it and every path under /data would resolve to nothing.
[ -d /data ] && [ ! -L /data ] && rmdir /data 2>/dev/null
ln -sfn /mnt/userdata/cat-s22-data /data
