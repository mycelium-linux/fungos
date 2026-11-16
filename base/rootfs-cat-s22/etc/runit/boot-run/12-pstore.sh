#!/bin/sh
# Archive the previous boot's ramoops records, then clear the backend.
#
# ramoops lives in reserved RAM, not on a block device: the only way to read it
# is to mount pstore, and the only way to free a record is to unlink it through
# that mount (ramoops_pstore_erase zaps the zone). Nothing else drains it, so
# without this hook /sys/fs/pstore is an empty directory, every record the last
# boot left behind is invisible, and the zones are never released.
#
# Records only appear at mount time: pstore.update_ms is -1, so a crash written
# after this boot started stays invisible until the next boot. That matches how
# this runs -- once per boot, before anything else looks at the region.
#
# Nothing here is fatal: a boot with no persistent store, no pstore support, or
# no records is a normal boot and must still reach the userland.
set -u

say() {
	# The boot-run stage has no logger worth the name; dmesg is where the rest
	# of the boot narrative lives, and /data/pstore/last.log survives the boot.
	printf 'pstore: %s\n' "$*" > /dev/kmsg 2>/dev/null || true
	if [ -n "${log:-}" ]; then
		printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >>"$log" 2>/dev/null || true
	fi
}

dest=/data/pstore
[ -d /data ] || exit 0
mkdir -p "$dest" || exit 0
log=$dest/last.log

# The record names carry the reason the previous boot ended, which is the most
# useful thing to file an archive under: dmesg records start with "Oops#1" or
# "Panic#1" depending on kmsg_dump_reason. Fall back to the record type.
archive_reason() {
	reason=boot
	for rec in "$1"/*; do
		[ -f "$rec" ] || continue
		case ${rec##*/} in
		dmesg-ramoops-*)
			first=$(head -n 1 "$rec" 2>/dev/null)
			case $first in
			Panic*) reason=panic ;;
			Oops*)  reason=oops ;;
			esac
			;;
		console-ramoops-*)
			[ "$reason" = boot ] && reason=console
			;;
		esac
	done
}

mnt=/run/pstore
mkdir -p "$mnt"
if ! mountpoint -q "$mnt"; then
	# pstore hands back a read-only mount even when asked for rw, and the unlink
	# that frees a zone needs a writable one, so remount before archiving. If the
	# remount fails the archive still happens, it just cannot clear.
	mount -t pstore pstore "$mnt" 2>/dev/null || {
		say "cannot mount pstore; ramoops records stay unread this boot"
		exit 0
	}
	mount -o remount,rw "$mnt" 2>/dev/null || true
fi

set -- "$mnt"/*
if [ ! -e "$1" ]; then
	say "no records in ramoops"
	umount "$mnt" 2>/dev/null || true
	exit 0
fi

archive_reason "$mnt"

# A counter, not the timestamp: this phone has no RTC, so every boot stamps
# 1970-01-01 and a timestamp alone would overwrite yesterday's archive.
seq=$(cat "$dest/.seq" 2>/dev/null || echo 0)
n=$((seq + 1))
echo "$n" > "$dest/.seq" 2>/dev/null || true
out=$dest/$(printf '%04d' "$n")-$reason-$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$out" || exit 0

kept=0
erased=0
for rec in "$mnt"/*; do
	[ -f "$rec" ] || continue
	name=${rec##*/}
	if ! cp -f "$rec" "$out/$name"; then
		# Never erase what we did not copy: the region is the only copy.
		say "ERROR copying $name; record left in ramoops"
		continue
	fi
	kept=$((kept + 1))
	if rm -f "$rec"; then
		erased=$((erased + 1))
	else
		say "ERROR erasing $name; backend keeps it (archive in $out)"
	fi
done

umount "$mnt" 2>/dev/null || true
say "archived $kept record(s) to $out, erased $erased"
