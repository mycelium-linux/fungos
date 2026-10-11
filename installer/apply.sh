#!/bin/sh
set -eu
export DEBIAN_FRONTEND=noninteractive
payload=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
case "$#:$*" in
  0:) ;;
  1:--installer)
    # Own the outer d-i transition here: in-target unmounts its temporary /run
    # before the installer's finishing APT operations access the target again.
    [ -f /target/etc/debian_version ] || { echo 'installer target missing' >&2; exit 1; }
    [ ! -e /target/tmp/fungos-installer ] || { echo 'installer staging already exists' >&2; exit 1; }
    cp -a "$payload" /target/tmp/fungos-installer
    in-target /bin/sh /tmp/fungos-installer/apply.sh
    mkdir -p /target/run/systemd/resolve
    cp -L /etc/resolv.conf /target/run/systemd/resolve/stub-resolv.conf
    exit 0
    ;;
  *) echo 'usage: apply.sh [--installer]' >&2; exit 1 ;;
esac
. "$payload/release.env"
# d-i's base system comes from the locked ISO; converge all installed packages to
# the same signed snapshot as base/rootfs builds before copying common hooks.
printf 'Acquire::Check-Valid-Until "false";\nAPT::Install-Recommends "false";\n' > /etc/apt/apt.conf.d/90-fungos
printf 'deb https://snapshot.debian.org/archive/debian/%s %s main\ndeb https://snapshot.debian.org/archive/debian/%s %s-updates main\ndeb https://snapshot.debian.org/archive/debian-security/%s %s-security main\n' \
 "$DEBIAN_SNAPSHOT" "$DEBIAN_SUITE" "$DEBIAN_SNAPSHOT" "$DEBIAN_SUITE" "$DEBIAN_SNAPSHOT" "$DEBIAN_SUITE" > /etc/apt/sources.list
rm -f /etc/apt/sources.list.d/debian.sources
apt-get update
apt-get -y --no-install-recommends dist-upgrade
xargs apt-get install -y --no-install-recommends < "$payload/packages"
if [ -d "$payload/components" ]; then
  (cd "$payload/components" && sha256sum -c SHA256SUMS)
  # APT resolves runtime dependencies from the same signed Debian snapshot.
  # Owner packages must not contain maintainer scripts that auto-enroll/start.
  apt-get install -y --no-install-recommends "$payload"/components/*.deb
  mkdir -p /usr/share/fungos/components
  cp "$payload/components/components.tsv" "$payload/components/SHA256SUMS" /usr/share/fungos/components/
  cp "$payload/runtime-components" /usr/share/fungos/components/runtime-components
  case "$(cat "$payload/runtime-components")" in
    owner-debs) systemctl enable mycelium.service unibus-router.service ;;
    mycelium-only-qualification) systemctl enable mycelium.service ;;
    *) echo 'unknown component purpose; refusing service activation' >&2; exit 1 ;;
  esac
fi
sh "$payload/apply-overlay.sh" "$payload/rootfs" /
chmod 0755 /usr/libexec/fungos-first-contact /usr/libexec/fungos-verify-update
mkdir -p /usr/lib/fungos/first-contact.d /usr/lib/fungos/update-verifiers.d /usr/share/fungos/installer
systemctl enable systemd-networkd systemd-resolved ssh fungos-first-contact
# The outer installer mode restores finishing DNS after in-target returns.
# At first boot resolved owns this volatile path; no installer DNS lives in /etc.
rm -f /etc/resolv.conf
ln -s /run/systemd/resolve/stub-resolv.conf /etc/resolv.conf
dpkg-query -W -f='${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\t${Architecture}\n' | LC_ALL=C sort > /usr/share/fungos/installer/installed.tsv
cp "$payload/release.env" /usr/share/fungos/installer/
find /etc/ssh -maxdepth 1 -type f -name 'ssh_host_*' -delete
: > /etc/machine-id
rm -f /var/lib/dbus/machine-id
rm -rf /tmp/fungos-installer
