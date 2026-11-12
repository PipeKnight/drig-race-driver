"""Build the native macOS game and package a code-only DRIG.app (docs/MACOS.md).

Optionally embed private game data and create a portable DMG.
"""

import argparse
import json
from pathlib import Path
import plistlib
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]

# Finder supplies no command line or useful working directory. Keep user data
# and diagnostics outside the bundle, including in the menu's child processes.
LAUNCHER = '''#!/bin/sh
set -eu
macos_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data_dir="${GRID_DATA_DIR:-$HOME/Library/Application Support/GRID Reassembly}"
mkdir -p "$data_dir/playtest"
log=$(mktemp "$data_dir/playtest/macos.XXXXXX")
cd "$macos_dir/../Resources"
interactive=false
if [ "$#" -eq 0 ]; then interactive=true; set -- --menu; fi
if [ -d "$macos_dir/../Resources/Grid" ]; then
    # Menu screens read root from settings, while the race also accepts CLI root.
    set -- "$@" --settings "$macos_dir/../Resources/portable.ini" --game-root "$macos_dir/../Resources/Grid"
elif [ "$interactive" = true ]; then
    root_file="$data_dir/game-root.txt"
    game_root=""
    if [ -f "$root_file" ]; then IFS= read -r game_root < "$root_file" || true; fi
    if [ ! -f "$game_root/grid.exe" ] || [ ! -f "$game_root/cars/lac/lac.ned" ]; then
        game_root=$(/usr/bin/osascript -e 'POSIX path of (choose folder with prompt "Select your purchased Windows GRID 2008 folder (containing grid.exe and cars).")') || exit 0
        if [ ! -f "$game_root/grid.exe" ] || [ ! -f "$game_root/cars/lac/lac.ned" ]; then
            /usr/bin/osascript -e 'display alert "GRID game files not found" message "Select the complete Windows GRID 2008 installation folder, containing grid.exe and cars. No game data is included with DRIG."'
            exit 1
        fi
        printf '%s\\n' "$game_root" > "$root_file"
    fi
    # Menus read the game location from settings rather than CLI flags.
    printf '[game]\\nroot = %s\\n' "$game_root" > "$data_dir/game-location.ini"
    set -- "$@" --settings "$data_dir/game-location.ini" --game-root "$game_root"
fi
exec "$macos_dir/grid-app" "$@" --data-dir "$data_dir" >"$log" 2>&1
'''


def create_dmg(bundle, destination):
    """Create a read-only compressed image with the app and an Applications link."""
    bundle, destination = Path(bundle).resolve(), Path(destination).resolve()
    if destination.exists():
        raise FileExistsError(destination)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    private = ROOT / "local"
    private.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="dmg-stage-", dir=private) as stage:
        subprocess.run(["/bin/cp", "-cR", str(bundle), str(Path(stage) / "DRIG.app")], check=True)
        (Path(stage) / "Applications").symlink_to("/Applications")
        subprocess.run(["hdiutil", "create", "-volname", "GRID", "-srcfolder", stage,
                        "-format", "UDZO", str(destination)], check=True)
    subprocess.run(["hdiutil", "verify", str(destination)], check=True)
    print(f"Created {destination}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-build", action="store_true",
                        help="package an existing native release binary without rebuilding")
    parser.add_argument("--binary", type=Path, help="verified native executable to package with --no-build")
    parser.add_argument("--source-root", type=Path, help="source checkout that produced the executable")
    parser.add_argument("--game-root", type=Path,
                        help="embed your purchased Windows GRID folder; keep this build private")
    parser.add_argument("--output-dir", type=Path,
                        help="bundle directory (required with --game-root; must be inside ignored local/)")
    parser.add_argument("--dmg", type=Path, help="create a compressed DMG at this new path inside local/")
    args = parser.parse_args()
    source_root = args.source_root.resolve() if args.source_root else ROOT
    if args.binary and not args.no_build:
        parser.error("--binary requires --no-build")
    if sys.platform != "darwin":
        parser.error("run this script on macOS; Windows and Android use their own build tools")
    private = (ROOT / "local").resolve()
    for path in [args.output_dir, args.dmg]:
        if path and args.game_root and not path.resolve().is_relative_to(private):
            parser.error("bundles containing game data and their DMGs must stay under ignored local/")
    if args.game_root:
        args.game_root = args.game_root.resolve()
        if not args.output_dir:
            parser.error("--game-root requires --output-dir under ignored local/")
        for marker in ["grid.exe", "cars/lac/lac.ned", "database/database.bin", "database/schema.bin"]:
            if not (args.game_root / marker).is_file():
                parser.error(f"incomplete game folder: {marker} is missing")
    if args.dmg and args.dmg.exists():
        parser.error("DMG already exists; choose a new output path")
    if not args.no_build:
        subprocess.run(["cargo", "build", "--release", "--locked", "-p", "grid-app",
                        "--bin", "grid-app"], cwd=source_root, check=True)
    if args.binary:
        binary = args.binary.resolve()
        release = binary.parent
    else:
        metadata = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=source_root))
        release = Path(metadata["target_directory"]) / "release"
        binary = release / "grid-app"
    if not binary.is_file():
        parser.error(f"native release binary missing: {binary}; unset CARGO_BUILD_TARGET/build.target")
    subprocess.run(["lipo", str(binary), "-verify_arch", platform.machine()], check=True)
    bundle = (args.output_dir.resolve() if args.output_dir else release) / "DRIG.app"
    if args.game_root and bundle.exists():
        parser.error("private bundle already exists; use a fresh output directory")
    if not args.game_root and (bundle / "Contents/Resources/Grid").exists():
        parser.error("existing bundle contains private game data; use a fresh output directory")
    contents = bundle / "Contents"
    macos = contents / "MacOS"
    macos.mkdir(parents=True, exist_ok=True)
    (contents / "Resources").mkdir(exist_ok=True)
    if args.game_root:
        # APFS clone-on-write avoids a second physical copy of the 10 GB install.
        subprocess.run(["/bin/cp", "-cR", str(args.game_root),
                        str(contents / "Resources/Grid")], check=True)
        game = contents / "Resources/Grid"
        # Archive extraction often creates owner-only directories. The app
        # must also be readable after an administrator copies it to Applications.
        for path in [game, *game.rglob("*")]:
            if not path.is_symlink():
                path.chmod(0o755 if path.is_dir() else 0o644)
        (contents / "Resources/portable.ini").write_text("[game]\nroot = Grid\n", encoding="utf-8")
    shutil.copy2(binary, macos / "grid-app")
    launcher = macos / "DRIG"
    launcher.write_text(LAUNCHER, encoding="utf-8")
    launcher.chmod(0o755)
    icon = ROOT / "crates/grid-app/assets/macos/DRIG.icns"
    if icon.is_file():
        shutil.copy2(icon, contents / "Resources/DRIG.icns")
    shutil.copy2(source_root / "LICENSE", contents / "Resources/LICENSE.txt")
    third_party_licenses = source_root / "LICENSES"
    if third_party_licenses.is_dir():
        shutil.copytree(third_party_licenses, contents / "Resources/LICENSES", dirs_exist_ok=True)
    shutil.copy2(source_root / "crates/grid-app/assets/fonts/FiraMono-LICENSE",
                 contents / "Resources/FiraMono-LICENSE.txt")
    with (contents / "Info.plist").open("wb") as out:
        plistlib.dump({
            "CFBundleExecutable": "DRIG",
            "CFBundleIdentifier": "org.reassembly.drig",
            "CFBundleName": "DRIG",
            "CFBundleDisplayName": "DRIG",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "0.1.0",
            **({"CFBundleIconFile": "DRIG.icns"} if icon.is_file() else {}),
            "NSHighResolutionCapable": True,
        }, out)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source_root, text=True).strip()
    (contents / "Resources/build.json").write_text(json.dumps({
        "source_revision": revision, "architecture": platform.machine(),
        "bundled_game_data": bool(args.game_root),
    }, indent=2) + "\n", encoding="utf-8")
    # Local ad-hoc signing needs no Apple account. This is not notarization.
    subprocess.run(["codesign", "--force", "--sign", "-", str(bundle)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)
    print(f"Created {bundle}")
    print("PRIVATE: includes your purchased game data." if args.game_root else "No original game files included.")
    if args.dmg:
        create_dmg(bundle, args.dmg)


if __name__ == "__main__":
    main()
