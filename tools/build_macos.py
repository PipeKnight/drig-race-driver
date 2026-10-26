"""Build the native macOS game and package a code-only DRIG.app (docs/MACOS.md)."""

import argparse
import json
from pathlib import Path
import plistlib
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

# Finder supplies no command line or useful working directory. Keep user data
# and diagnostics outside the bundle, including in the menu's child processes.
LAUNCHER = '''#!/bin/sh
set -eu
macos_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_dir="$HOME/Library/Application Support/GRID Reassembly"
mkdir -p "$data_dir/playtest"
log=$(mktemp "$data_dir/playtest/macos.XXXXXX")
cd "$macos_dir/../Resources"
exec "$macos_dir/grid-app" --menu --data-dir "$data_dir" "$@" >"$log" 2>&1
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true",
                        help="package an existing native release binary without rebuilding")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("run this script on macOS; Windows and Android use their own build tools")
    if not args.no_build:
        subprocess.run(["cargo", "build", "--release", "--locked", "-p", "grid-app",
                        "--bin", "grid-app"], cwd=ROOT, check=True)
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT))
    release = Path(metadata["target_directory"]) / "release"
    binary = release / "grid-app"
    if not binary.is_file():
        parser.error(f"native release binary missing: {binary}; unset CARGO_BUILD_TARGET/build.target")
    subprocess.run(["lipo", str(binary), "-verify_arch", platform.machine()], check=True)
    bundle = release / "DRIG.app"
    contents = bundle / "Contents"
    macos = contents / "MacOS"
    macos.mkdir(parents=True, exist_ok=True)
    (contents / "Resources").mkdir(exist_ok=True)
    shutil.copy2(binary, macos / "grid-app")
    launcher = macos / "DRIG"
    launcher.write_text(LAUNCHER, encoding="utf-8")
    launcher.chmod(0o755)
    with (contents / "Info.plist").open("wb") as out:
        plistlib.dump({
            "CFBundleExecutable": "DRIG",
            "CFBundleIdentifier": "org.reassembly.drig",
            "CFBundleName": "DRIG",
            "CFBundleDisplayName": "DRIG",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "0.1.0",
            "NSHighResolutionCapable": True,
        }, out)
    # Local ad-hoc signing needs no Apple account. This is not notarization.
    subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
    print(f"Created {bundle}\nNo original game files are included. See docs/MACOS.md.")


if __name__ == "__main__":
    main()
