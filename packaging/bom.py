#!/usr/bin/env python3
"""Read the packaging release set without teaching the shell JSON."""

import json
import sys


def fail(message):
    print(f"error: {message}", file=sys.stderr)
    raise SystemExit(1)


def main():
    if len(sys.argv) < 3:
        fail("usage: bom.py COMMAND PATH [ARG]")
    command, path, *args = sys.argv[1:]
    with open(path, encoding="utf-8") as handle:
        data = json.load(handle)

    if command == "field":
        value = data
        for key in args[0].split("."):
            value = value[key]
        print(value)
    elif command == "targets":
        targets = sorted({target for component in data["components"] for target in component["targets"]})
        print("\n".join(targets))
    elif command == "components":
        target = args[0]
        found = False
        for component in data["components"]:
            entry = component["targets"].get(target)
            if entry is None:
                continue
            found = True
            print("\t".join([
                component["name"],
                component["version"],
                entry["archive"],
                entry["url"],
                entry["sha256"],
            ]))
        if not found:
            fail(f"no components for target {target}")
    elif command == "binaries":
        component = next((c for c in data["components"] if c["name"] == args[0]), None)
        if component is None:
            fail(f"unknown component {args[0]}")
        print("\n".join(component.get("binaries", [])))
    elif command == "licenses":
        component = next((c for c in data["components"] if c["name"] == args[0]), None)
        if component is None:
            fail(f"unknown component {args[0]}")
        print("\n".join(component.get("licenses", [])))
    else:
        fail(f"unknown command {command}")


if __name__ == "__main__":
    main()
