#!/bin/sh
# The initramfs mounts userdata before switch_root; that mount does not survive,
# so the userland owns it from here. Without it /data is an empty directory and
# every log and state file underneath it silently lives in RAM.
mkdir -p /mnt/userdata
mountpoint -q /mnt/userdata 2>/dev/null || \
	mount -t f2fs /dev/mmcblk0p56 /mnt/userdata || echo "userdata: mount failed"
mkdir -p /mnt/userdata/cat-s22-data
ln -sfn /mnt/userdata/cat-s22-data /data
