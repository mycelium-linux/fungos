#!/bin/sh
# Both panels belong to userspace. fbcon binds to a framebuffer the moment it
# registers -- simpledrm's 60x40 console comes up at 0.2 s, long before
# userspace -- whether or not `console=` names it, so every kernel message
# after that paints over whatever the presentation stack just drew.
# `console=ttyGS0` keeps the log on the serial line; this takes the panels
# away from the VT and turns them on.
#
# Three details, all learned the hard way:
#  * the name sysfs reports is "(M) frame buffer device", not the bare name,
#  * fbcon binds again when the second panel registers, so wait for both
#    framebuffers before unbinding, and confirm it stuck,
#  * taking the VT away takes away the only thing that was doing a modeset on
#    the panels, and fbdev alone never enables a CRTC: the outer panel stayed
#    dark while writes to /dev/fb1 landed in its memory. A blank of 0 is what
#    makes the driver run its enable sequence and switch the backlight on.
i=0
while [ "$i" -lt 30 ]; do
    set -- /sys/class/graphics/fb*
    [ "$#" -ge 2 ] && break
    i=$((i + 1))
    sleep 0.1
done

attempt=0
while [ "$attempt" -lt 3 ]; do
    pending=0
    for console in /sys/class/vtconsole/vtcon*; do
        [ -r "$console/name" ] || continue
        case "$(cat "$console/name")" in
        *"frame buffer device"*)
            echo 0 > "$console/bind" 2>/dev/null
            [ "$(cat "$console/bind" 2>/dev/null)" = "0" ] || pending=1
            ;;
        esac
    done
    for blank in /sys/class/graphics/fb*/blank; do
        [ -w "$blank" ] || continue
        [ "$(cat "$blank" 2>/dev/null)" = "0" ] && continue
        echo 0 > "$blank" 2>/dev/null
        [ "$(cat "$blank" 2>/dev/null)" = "0" ] || pending=1
    done
    [ "$pending" = 0 ] && break
    attempt=$((attempt + 1))
    sleep 0.1
done
