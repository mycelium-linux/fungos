#!/bin/sh
# Nothing on this image brings loopback up, and a surprising amount needs it:
# dbus, myceliumd's socket clients, and any resolver on 127.0.0.1 start failing
# with "Network is unreachable" against ::1 or 127.0.0.1 while lo sits DOWN.
# It is also what gives the kernel its ::1 and 127.0.0.1 addresses, so it has to
# happen before the services that bind them.
ip link set lo up
