#!/bin/sh
# Debian's dnsmasq runit script disables itself unless the package marker
# exists; its postinst only knows about systemd, so on this image the service
# sits "down" forever. It also ships conf-dir commented out, so nothing in
# /etc/dnsmasq.d is read. Both are packaging assumptions, not configuration.
[ -x /usr/sbin/dnsmasq ] || exit 0
marker=/usr/share/dnsmasq/installed-marker
[ -e "$marker" ] || : > "$marker"
grep -q '^conf-dir=/etc/dnsmasq.d/,\*\.conf' /etc/dnsmasq.conf 2>/dev/null || \
    sed -i 's|^#conf-dir=/etc/dnsmasq.d/,\*\.conf|conf-dir=/etc/dnsmasq.d/,*.conf|' /etc/dnsmasq.conf
ln -sfn /etc/sv/dnsmasq /etc/runit/runsvdir/default/dnsmasq
ln -sfn /etc/sv/mycelium-dns /etc/runit/runsvdir/default/mycelium-dns
