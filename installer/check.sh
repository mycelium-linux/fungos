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
# Run the real outer mode against an isolated target. The stand-in in-target
# removes its temporary /run, reproducing d-i's unmount before finishing APT.
mkdir -p "$tmp/resolver/target/etc" "$tmp/resolver/target/tmp" "$tmp/resolver/etc" "$tmp/resolver/bin"
printf '13\n' > "$tmp/resolver/target/etc/debian_version"
printf 'nameserver 192.0.2.53\n' > "$tmp/resolver/etc/resolv.conf"
sed "s@/target@$tmp/resolver/target@g; s@/etc/resolv.conf@$tmp/resolver/etc/resolv.conf@g" "$dir/apply.sh" > "$tmp/resolver/apply.sh"
printf '#!/bin/sh\nset -eu\nrm -rf "%s/target/run" "%s/target/tmp/fungos-installer"\n' "$tmp/resolver" "$tmp/resolver" > "$tmp/resolver/bin/in-target"
chmod +x "$tmp/resolver/bin/in-target"
PATH="$tmp/resolver/bin:$PATH" sh "$tmp/resolver/apply.sh" --installer
[ "$(cat "$tmp/resolver/target/run/systemd/resolve/stub-resolv.conf")" = 'nameserver 192.0.2.53' ]
# Invalid calls and stale staging must fail before invoking the target.
if sh "$tmp/resolver/apply.sh" unexpected > "$tmp/args.log" 2>&1; then
  echo 'invalid apply mode accepted' >&2; exit 1
fi
mkdir "$tmp/resolver/target/tmp/fungos-installer"
if PATH="$tmp/resolver/bin:$PATH" sh "$tmp/resolver/apply.sh" --installer > "$tmp/staging.log" 2>&1; then
  echo 'stale installer staging accepted' >&2; exit 1
fi
grep -q 'installer staging already exists' "$tmp/staging.log"
for preseed in interactive test; do
  grep -Fq '/bin/sh /cdrom/fungos/apply.sh --installer' "$dir/$preseed.preseed"
done
echo 'installer syntax, firmware rejection, positive disk guard, ambiguity rejection, identity rejection, public preseed safety, outer resolver transition: passed'
