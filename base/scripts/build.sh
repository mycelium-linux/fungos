#!/bin/sh
set -eu

base_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
# shellcheck source=/dev/null
. "$base_dir/config/release.env"

arch=${1:-}
profile=${2:-base}
case "$arch" in amd64|arm64) ;; *) echo "usage: $0 {amd64|arm64} PROFILE" >&2; exit 2;; esac
[ -f "$base_dir/profiles/$profile.capabilities" ] || {
  echo "unsupported profile: $profile (no profiles/$profile.capabilities)" >&2
  exit 2
}
init=systemd
if [ -f "$base_dir/profiles/$profile.init" ]; then
  init=$(tr -d '[:space:]' < "$base_dir/profiles/$profile.init")
fi
case "$init" in
  systemd|runit) ;;
  *) echo "unsupported init for $profile: $init" >&2; exit 2;;
esac

command -v mmdebstrap >/dev/null || { echo "mmdebstrap is required" >&2; exit 1; }
debian_keyring=/usr/share/keyrings/debian-archive-keyring.gpg
[ -r "$debian_keyring" ] || {
  echo "Debian archive keyring is required at $debian_keyring" >&2
  exit 1
}
out_dir=${OUT_DIR:-"$base_dir/out"}
work_dir=${WORK_DIR:-"$base_dir/.work/$profile/$arch"}
rootfs="$work_dir/rootfs"
artifact="$out_dir/fungos-$profile-$arch.tar"
package_file="$out_dir/fungos-$profile-$arch.packages"
manifest="$out_dir/fungos-$profile-$arch.manifest"
inventory="$out_dir/fungos-$profile-$arch.installed.tsv"
release_inputs="$out_dir/fungos-$profile-$arch.release-inputs"

mkdir -p "$out_dir" "$work_dir"
[ ! -e "$rootfs" ] || { echo "refusing non-clean work directory: $rootfs" >&2; exit 1; }
overlay=${CAPABILITY_OVERLAY:-}
"$base_dir/scripts/resolve-packages.sh" "$base_dir/profiles/$profile.capabilities" "$overlay" > "$package_file"
packages=$(paste -sd, "$package_file")
mirror="deb [check-valid-until=no] https://snapshot.debian.org/archive/debian/$DEBIAN_SNAPSHOT $DEBIAN_SUITE main"
updates_mirror="deb [check-valid-until=no] https://snapshot.debian.org/archive/debian/$DEBIAN_SNAPSHOT $DEBIAN_SUITE-updates main"
security_mirror="deb [check-valid-until=no] https://snapshot.debian.org/archive/debian-security/$DEBIAN_SNAPSHOT $DEBIAN_SUITE-security main"

mmdebstrap \
  --skip=cleanup/apt/lists \
  --variant=minbase \
  --keyring="$debian_keyring" \
  --architectures="$arch" \
  --include="$packages" \
  --aptopt='Acquire::Languages "none"' \
  --aptopt='APT::Install-Recommends "false"' \
  --aptopt='APT::Install-Suggests "false"' \
  --dpkgopt='path-exclude=/usr/share/doc/*' \
  --dpkgopt='path-include=/usr/share/doc/*/copyright' \
  --dpkgopt='path-exclude=/usr/share/man/*' \
  --dpkgopt='path-exclude=/usr/share/locale/*' \
  "$DEBIAN_SUITE" "$rootfs" "$mirror" "$updates_mirror" "$security_mirror"

# APT validates signed release metadata and package hashes. Keep its exact
# authenticated inputs alongside source/version inventory before sanitizing.
chroot "$rootfs" dpkg-query -W -f='${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\t${Architecture}\n' \
  | LC_ALL=C sort > "$inventory"
[ -s "$inventory" ] || { echo 'installed package inventory is empty' >&2; exit 1; }
mkdir -p "$release_inputs"
find "$rootfs/var/lib/apt/lists" -maxdepth 1 -type f -name '*InRelease' -exec cp {} "$release_inputs/" \;
[ "$(find "$release_inputs" -type f -name '*InRelease' | wc -l)" -eq 3 ] || { echo 'expected three authenticated archive releases' >&2; exit 1; }
(cd "$release_inputs" && sha256sum *InRelease) > "$release_inputs/SHA256SUMS"

# The services Debian itself ships stay as they are; anything the profile
# overlay or a runtime component adds is enabled below. The snapshot has to be
# taken before the overlay lands, or the overlay's own services are mistaken for
# Debian's and never enabled.
base_services=$(ls "$rootfs/etc/sv" 2>/dev/null || true)

sh "$base_dir/scripts/apply-overlay.sh" "$base_dir/rootfs" "$rootfs"
if [ -d "$base_dir/rootfs-$profile" ]; then
  sh "$base_dir/scripts/apply-overlay.sh" "$base_dir/rootfs-$profile" "$rootfs"
fi
# Runtime components are owner-built .debs (see packages/COMPONENTS.md), not
# overlay payloads: unpack them so their files land like a package would,
# without running maintainer scripts.
if [ -n "${COMPONENTS_DIR:-}" ]; then
  for deb in "$COMPONENTS_DIR"/*.deb; do
    [ -f "$deb" ] || continue
    dpkg-deb -x "$deb" "$rootfs"
    printf '%s %s\n' "$(dpkg-deb -f "$deb" Package)" "$(dpkg-deb -f "$deb" Version)" >> "$manifest.components"
  done
fi
chmod 0755 "$rootfs/usr/libexec/fungos-first-contact" "$rootfs/usr/libexec/fungos-verify-update"
mkdir -p "$rootfs/usr/lib/fungos/first-contact.d" "$rootfs/usr/lib/fungos/update-verifiers.d"

if [ "$init" = systemd ]; then
  mkdir -p \
    "$rootfs/etc/systemd/system/multi-user.target.wants" \
    "$rootfs/etc/systemd/system/network-online.target.wants"
  ln -sfn /run/systemd/resolve/stub-resolv.conf "$rootfs/etc/resolv.conf"
  ln -sfn /usr/lib/systemd/system/systemd-networkd.service "$rootfs/etc/systemd/system/multi-user.target.wants/systemd-networkd.service"
  ln -sfn /usr/lib/systemd/system/systemd-resolved.service "$rootfs/etc/systemd/system/multi-user.target.wants/systemd-resolved.service"
  ln -sfn /usr/lib/systemd/system/systemd-networkd-wait-online.service "$rootfs/etc/systemd/system/network-online.target.wants/systemd-networkd-wait-online.service"
  ln -sfn /usr/lib/systemd/system/ssh.service "$rootfs/etc/systemd/system/multi-user.target.wants/ssh.service"
  ln -sfn /usr/lib/systemd/system/fungos-first-contact.service "$rootfs/etc/systemd/system/multi-user.target.wants/fungos-first-contact.service"
else
  # runit: Debian points /etc/service at /etc/runit/runsvdir/current, so the
  # services are enabled inside that runsvdir. Only the profile's own services
  # are enabled; the run scripts wait on the hardware, socket or config they
  # need instead of declaring a dependency graph.
  runsvdir=default
  if [ -r "$rootfs/etc/runit/default.runsvdir" ]; then
    runsvdir=$(tr -d '[:space:]' < "$rootfs/etc/runit/default.runsvdir")
  fi
  if [ -d "$rootfs/etc/sv" ]; then
    mkdir -p "$rootfs/etc/runit/runsvdir/$runsvdir"
    find "$rootfs/etc/sv" -name run -exec chmod 0755 {} +
    if [ -d "$rootfs/etc/runit/boot-run" ]; then
      find "$rootfs/etc/runit/boot-run" -name '*.sh' -exec chmod 0755 {} +
    fi
    for service in "$rootfs"/etc/sv/*; do
      [ -d "$service" ] || continue
      name=$(basename "$service")
      [ -f "$service/run" ] || continue
      printf '%s\n' "$base_services" | grep -Fx "$name" >/dev/null && continue
      ln -sfn "/etc/sv/$name" "$rootfs/etc/runit/runsvdir/$runsvdir/$name"
    done
  fi
  printf '# Rewritten at boot by the network service (dhcpcd).\n' > "$rootfs/etc/resolv.conf"
fi

# Remove host-specific and time-varying state. systemd and ssh recreate these on
# first boot; package indexes are release inputs, not runtime image content.
find "$rootfs/etc/ssh" -maxdepth 1 -type f -name 'ssh_host_*' -delete
find "$rootfs/var/log" -type f -exec truncate -s 0 {} +
find "$rootfs/var/lib/apt/lists" -mindepth 1 -delete
: > "$rootfs/etc/machine-id"

find "$rootfs" -xdev -print0 | xargs -0 touch --no-dereference --date="@$SOURCE_DATE_EPOCH"
tar --sort=name --format=posix --numeric-owner --owner=0 --group=0 \
  --mtime="@$SOURCE_DATE_EPOCH" --pax-option=delete=atime,delete=ctime \
  -C "$rootfs" -cf "$artifact" .

{
  echo "architecture=$arch"
  echo "debian_suite=$DEBIAN_SUITE"
  echo "debian_snapshot=$DEBIAN_SNAPSHOT"
  echo "source_date_epoch=$SOURCE_DATE_EPOCH"
  echo "profile=$profile"
  echo "artifact=$(basename "$artifact")"
  echo "artifact_sha256=$(sha256sum "$artifact" | cut -d' ' -f1)"
  echo "packages_sha256=$(sha256sum "$package_file" | cut -d' ' -f1)"
  echo "installed_sha256=$(sha256sum "$inventory" | cut -d' ' -f1)"
  echo "release_inputs_sha256=$(sha256sum "$release_inputs/SHA256SUMS" | cut -d' ' -f1)"
  echo "archive_suites=$DEBIAN_SUITE,$DEBIAN_SUITE-updates,$DEBIAN_SUITE-security"
} > "$manifest"
(cd "$out_dir" && sha256sum "$(basename "$artifact")") > "$artifact.sha256"
echo "built $artifact"
