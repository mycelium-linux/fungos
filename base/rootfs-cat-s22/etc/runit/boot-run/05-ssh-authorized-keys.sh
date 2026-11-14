#!/bin/sh
# The initramfs parks the operator's authorized_keys on userdata before it
# switch_roots; a Debian image with no key on it is a machine nobody can log
# in to, so import it on every boot.
src=/mnt/userdata/cat-s22-data/ssh/authorized_keys
[ -r "$src" ] || exit 0
mkdir -p /root/.ssh
install -m 0600 "$src" /root/.ssh/authorized_keys
