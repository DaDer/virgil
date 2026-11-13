#!/usr/bin/env python3
"""Set or check Virgil's version number.

The version lives in two files: Cargo.toml ([workspace.package]) and package.json. The app bundle
takes its version from Cargo.toml (tauri.conf.json has no version of its own).

  tools/set-version.py 0.2.0          write 0.2.0 to both files (then run the checks and commit)
  tools/set-version.py --check        fail if the two files disagree
  tools/set-version.py --check v0.2.0 also fail if they are not 0.2.0 (a leading "v" is ignored)
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CARGO = ROOT / "Cargo.toml"
PACKAGE = ROOT / "package.json"
LOCK = ROOT / "package-lock.json"
CARGO_RE = re.compile(r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")([^"]+)(")', re.S)
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")


def cargo_version():
    m = CARGO_RE.search(CARGO.read_text())
    if not m:
        sys.exit("Cargo.toml: no version under [workspace.package]")
    return m.group(2)


def main():
    args = sys.argv[1:]
    if not args or args[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    if args[0] == "--check":
        cargo, package = cargo_version(), json.loads(PACKAGE.read_text())["version"]
        if cargo != package:
            sys.exit(f"versions disagree: Cargo.toml {cargo}, package.json {package}")
        want = args[1].lstrip("v") if len(args) > 1 and args[1] else None
        if want and want != cargo:
            sys.exit(f"the tag says {want} but the files say {cargo}; run tools/set-version.py {want} first")
        print(f"version {cargo}")
        return 0

    version = args[0].lstrip("v")
    if not VERSION_RE.match(version):
        sys.exit(f"not a version number: {args[0]} (expected something like 0.2.0)")
    CARGO.write_text(CARGO_RE.sub(lambda m: m.group(1) + version + m.group(3), CARGO.read_text(), count=1))
    for path in (PACKAGE, LOCK):
        if not path.exists():
            continue
        text = path.read_text()
        data = json.loads(text)
        old = data["version"]
        # Replace only the first occurrences (the project's own entries), keeping the file's formatting.
        count = 2 if path == LOCK else 1
        path.write_text(text.replace(f'"version": "{old}"', f'"version": "{version}"', count))
    print(f"version set to {version}; run `cargo check` to refresh Cargo.lock, then commit and tag v{version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
