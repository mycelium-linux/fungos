#!/bin/sh
set -eu
dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
for file in "$dir"/*.sh; do sh -n "$file"; done
if sh "$dir/build.sh" absent.iso absent-output.iso absent-bundle unknown-mode >/dev/null 2>&1; then
  echo 'unknown component mode accepted' >&2; exit 1
fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
if sh "$dir/qemu-proof.sh" nonexistent.iso "$tmp/proof" unsupported > "$tmp/firmware.log" 2>&1; then
  echo 'invalid firmware accepted' >&2; exit 1
fi
grep -q 'firmware must be bios or uefi' "$tmp/firmware.log"
[ ! -e "$tmp/proof" ]
if FUNGOS_OVMF_CODE="$tmp/missing-code" FUNGOS_OVMF_VARS="$tmp/missing-vars" \
  sh "$dir/qemu-proof.sh" nonexistent.iso "$tmp/proof" uefi > "$tmp/ovmf.log" 2>&1; then
  echo 'missing OVMF accepted' >&2; exit 1
fi
grep -q 'OVMF code/variables missing' "$tmp/ovmf.log"
[ ! -e "$tmp/proof" ]
mkdir -p "$tmp/block/vda/device"
printf 'FUNGOS_DISPOSABLE\n' > "$tmp/block/vda/device/serial"
printf '16777216\n' > "$tmp/block/vda/size"
FUNGOS_TEST_SYS_BLOCK="$tmp/block" sh "$dir/guard-disk.sh"
mkdir "$tmp/block/vdb"
if FUNGOS_TEST_SYS_BLOCK="$tmp/block" sh "$dir/guard-disk.sh" 2>/dev/null; then echo 'ambiguous disk accepted' >&2; exit 1; fi
rmdir "$tmp/block/vdb"
printf 'REAL_DISK\n' > "$tmp/block/vda/device/serial"
if FUNGOS_TEST_SYS_BLOCK="$tmp/block" sh "$dir/guard-disk.sh" 2>/dev/null; then echo 'wrong identity accepted' >&2; exit 1; fi
if grep -Eq 'partman|password|authorized_keys|claim|token' "$dir/interactive.preseed"; then
  # Comments describe absence; examine only actual answers.
  if sed '/^#/d' "$dir/interactive.preseed" | grep -Eq 'partman|password|authorized_keys|claim|token'; then echo 'unsafe public preseed' >&2; exit 1; fi
fi
if [ -n "${DEBIAN_TEST_ISO:-}" ]; then
  mkdir -p "$tmp/source/fungOS/installer"
  cp -a "$dir/../base" "$tmp/source/fungOS/base"
  cp "$dir/build.sh" "$tmp/source/fungOS/installer/"
  : > "$tmp/source/fungOS/base/rootfs/.env"
  if sh "$tmp/source/fungOS/installer/build.sh" "$DEBIAN_TEST_ISO" "$tmp/unsafe.iso" > "$tmp/rejection.log" 2>&1; then
    echo 'credential overlay accepted' >&2; exit 1
  fi
  grep -q 'credential state found' "$tmp/rejection.log"
  [ ! -e "$tmp/unsafe.iso" ]
fi
# Public and private installations must share profile package policy. In
# particular, d-i must not upgrade against an unpinned mirror before apply.sh.
grep -qx 'd-i apt-setup/use_mirror boolean false' "$dir/interactive.preseed"
grep -qx 'd-i apt-setup/services-select multiselect' "$dir/interactive.preseed"
grep -qx 'tasksel tasksel/first multiselect' "$dir/interactive.preseed"
grep -qx 'd-i pkgsel/upgrade select none' "$dir/interactive.preseed"
if sed '/^#/d' "$dir/test.preseed" | grep -Eq 'apt-setup/|tasksel/first|pkgsel/upgrade'; then
  echo 'private harness duplicates public package policy' >&2; exit 1
fi
grep -Fq 'cat "$script_dir/interactive.preseed"' "$dir/qemu-proof.sh"
# Exercise the actual resolver transition in an isolated filesystem. The
# installer must retain working DNS while leaving /etc pointed at volatile /run.
mkdir -p "$tmp/resolver/etc" "$tmp/resolver/payload"
printf 'nameserver 192.0.2.53\n' > "$tmp/resolver/etc/resolv.conf"
{
  grep -F 'cp -L /etc/resolv.conf "$payload/installer-resolv.conf"' "$dir/apply.sh"
  sed -n '/^mkdir -p \/run\/systemd\/resolve$/,/^ln -s \/run\/systemd\/resolve\/stub-resolv.conf \/etc\/resolv.conf$/p' "$dir/apply.sh"
} |
  sed "s@/etc/resolv.conf@$tmp/resolver/etc/resolv.conf@g; s@/run/systemd/resolve@$tmp/resolver/run/systemd/resolve@g" > "$tmp/resolver-transition.sh"
payload="$tmp/resolver/payload" sh "$tmp/resolver-transition.sh"
[ -L "$tmp/resolver/etc/resolv.conf" ]
[ "$(cat "$tmp/resolver/etc/resolv.conf")" = 'nameserver 192.0.2.53' ]
[ "$(readlink "$tmp/resolver/etc/resolv.conf")" = "$tmp/resolver/run/systemd/resolve/stub-resolv.conf" ]
# Also exercise a preexisting stub symlink, not just d-i's regular resolver.
payload="$tmp/resolver/payload" sh "$tmp/resolver-transition.sh"
[ "$(cat "$tmp/resolver/etc/resolv.conf")" = 'nameserver 192.0.2.53' ]
# Model resolved's maintainer script replacing the resolver between the early
# capture and late transition. The usable installer DNS must win, not its
# unstarted loopback stub. Run the real early/late command fragments separately.
grep -F 'cp -L /etc/resolv.conf "$payload/installer-resolv.conf"' "$dir/apply.sh" |
  sed "s@/etc/resolv.conf@$tmp/resolver/etc/resolv.conf@g" > "$tmp/resolver-capture.sh"
payload="$tmp/resolver/payload" sh "$tmp/resolver-capture.sh"
printf 'nameserver 127.0.0.53\n' > "$tmp/resolver/run/systemd/resolve/stub-resolv.conf"
sed -n '/^mkdir -p \/run\/systemd\/resolve$/,/^ln -s \/run\/systemd\/resolve\/stub-resolv.conf \/etc\/resolv.conf$/p' "$dir/apply.sh" |
  sed "s@/etc/resolv.conf@$tmp/resolver/etc/resolv.conf@g; s@/run/systemd/resolve@$tmp/resolver/run/systemd/resolve@g" > "$tmp/resolver-late.sh"
payload="$tmp/resolver/payload" sh "$tmp/resolver-late.sh"
[ "$(cat "$tmp/resolver/etc/resolv.conf")" = 'nameserver 192.0.2.53' ]
echo 'installer syntax, firmware rejection, positive disk guard, ambiguity rejection, identity rejection, public preseed safety: passed'
