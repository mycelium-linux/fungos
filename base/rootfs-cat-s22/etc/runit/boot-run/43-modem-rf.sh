#!/bin/sh
# Turn the modem's RF on once it has finished booting.
#
# The firmware comes up in CFUN=0: registered to nothing, +CSQ: 99,99. Setting
# CFUN=1 brings it online; it then camps on LTE and reports a real signal.
# 41-modem.sh starts the firmware, but the AT port only appears once the modem
# has loaded its image and its EFS, so wait for the port rather than sleep.
set -u

i=0
while [ ! -e /dev/wwan0at0 ] && [ "$i" -lt 120 ]; do
    i=$((i + 1))
    sleep 0.5
done
[ -e /dev/wwan0at0 ] || exit 0

python3 - <<'PY'
import os
import select
import time
import tty

fd = os.open('/dev/wwan0at0', os.O_RDWR | os.O_NOCTTY)
tty.setraw(fd)


def cmd(line, wait=3.0):
    os.write(fd, line.encode() + b'\r')
    buf, end = b'', time.time() + wait
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.3)
        if not ready:
            continue
        buf += os.read(fd, 4096)
        end = time.time() + 1.0
    return buf.decode(errors='replace').strip().replace('\r\n', ' ')


print('modem RF:', cmd('AT+CFUN=1', 15), '|', cmd('AT+CREG?'), '|', cmd('AT+COPS?'))
os.close(fd)
PY
