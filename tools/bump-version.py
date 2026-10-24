#!/usr/bin/env python3
"""Bump the launcher version by 0.01 and write it to every manifest.

The launcher is in beta and its version counts in steps of 0.01:
    v0.1 Beta → v0.11 Beta → v0.12 Beta → ... → v0.19 Beta → v0.2 Beta

    ./tools/bump-version.py           # bump by 0.01
    ./tools/bump-version.py --show    # print the current version only

Versions are stored as semver (`0.1.0`) because Tauri/Cargo require three
parts; the two-part beta number is what the UI shows.
"""

import json
import re
import sys
from decimal import Decimal
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
STEP = Decimal("0.01")

TAURI = ROOT / "apps/app/tauri.conf.json"
CARGO = ROOT / "Cargo.toml"
NPM = ROOT / "apps/app-frontend/package.json"
PKGBUILD = ROOT / "packaging/PKGBUILD"


def read_version() -> str:
    return json.loads(TAURI.read_text(encoding="utf-8"))["version"]


def beta_number(version: str) -> Decimal:
    """`0.1.0` → 0.10 so adding 0.01 gives 0.11 (the beta counter)."""
    major, minor, patch = (version.split(".") + ["0", "0"])[:3]
    return Decimal(f"{major}.{minor}") + Decimal(patch) / 100


def semver(value: Decimal) -> str:
    text = format(value.normalize(), "f")
    major, _, minor = text.partition(".")
    return f"{major}.{minor or '0'}.0"


def write(version: str) -> None:
    tauri = json.loads(TAURI.read_text(encoding="utf-8"))
    tauri["version"] = version
    TAURI.write_text(json.dumps(tauri, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    npm = json.loads(NPM.read_text(encoding="utf-8"))
    npm["version"] = version
    NPM.write_text(json.dumps(npm, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    cargo = CARGO.read_text(encoding="utf-8")
    cargo = re.sub(
        r'(^\[workspace\.package\][\s\S]*?^version = ")[^"]+(")',
        rf"\g<1>{version}\g<2>",
        cargo,
        count=1,
        flags=re.MULTILINE,
    )
    CARGO.write_text(cargo, encoding="utf-8")

    if PKGBUILD.exists():
        pkg = PKGBUILD.read_text(encoding="utf-8")
        pkg = re.sub(r"^pkgver=.*$", f"pkgver={version}", pkg, count=1, flags=re.MULTILINE)
        PKGBUILD.write_text(pkg, encoding="utf-8")


def label(version: str) -> str:
    number = beta_number(version)
    return f"v{format(number.normalize(), 'f')} Beta"


def main() -> int:
    current = read_version()
    if "--show" in sys.argv:
        print(f"{current}  ({label(current)})")
        return 0
    nxt = semver(beta_number(current) + STEP)
    write(nxt)
    print(f"{label(current)} → {label(nxt)}  (manifest version {nxt})")
    print("rebuild with ./build-package.sh to ship it")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
