#!/bin/bash
# Build the native Linux x86_64 player and package the code-only DRIG tarball and AppImage (docs/LINUX.md).
#
# Runs inside Linux from a clean source checkout on a Linux filesystem; on the build PC
# tools/package_linux.ps1 syncs a commit into WSL2 and runs this under the build lock.
#
#   tools/package_linux.sh --version 0.1.0 --out DIR --tools DIR [--no-build] [--allow-dirty]
#                          [--cargo-args "ARGS"]
#
# --tools DIR caches the pinned appimagetool and AppImage runtime (downloaded from their official
# GitHub releases when missing; every use checks the SHA-256 below). Writes into --out:
#   DRIG-vVERSION-linux-x86_64.tar.gz   folder with drig, LICENSE, LICENSES/, README.txt, build.json
#   DRIG-vVERSION-linux-x86_64.AppImage the same files in an AppImage (type 2, squashfs)
#   SHA256SUMS.txt                      lines for both, replacing older lines of the same names
# No game files are included. Ends with one VERDICT line.
set -euo pipefail

APPIMAGETOOL_VERSION=1.9.1
APPIMAGETOOL_URL=https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-x86_64.AppImage
APPIMAGETOOL_SHA256=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
RUNTIME_VERSION=20251108
RUNTIME_URL=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
RUNTIME_SHA256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

version=""; out=""; tools=""; build=1; allow_dirty=0; cargo_args=""
while [ $# -gt 0 ]; do
    case "$1" in
        --version) version=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        --tools) tools=$2; shift 2 ;;
        --no-build) build=0; shift ;;
        --allow-dirty) allow_dirty=1; shift ;;
        --cargo-args) cargo_args=$2; shift 2 ;;
        *) echo "package_linux: unknown argument $1" >&2; exit 2 ;;
    esac
done
[ -n "$version" ] && [ -n "$out" ] && [ -n "$tools" ] || {
    echo "usage: package_linux.sh --version V --out DIR --tools DIR [--no-build] [--allow-dirty] [--cargo-args ARGS]" >&2
    exit 2
}
[ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ] || { echo "VERDICT fail: needs Linux x86_64" >&2; exit 2; }

if ! command -v cargo >/dev/null && [ -f "$HOME/.cargo/env" ]; then
    . "$HOME/.cargo/env"
fi
for tool in cargo rustc git curl file readelf python3 sha256sum; do
    command -v "$tool" >/dev/null || { echo "VERDICT fail: $tool not found" >&2; exit 2; }
done

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"
if [ "$allow_dirty" = 0 ] && [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "VERDICT fail: source checkout has local changes (use --allow-dirty for a test package)" >&2
    exit 3
fi
revision=$(git rev-parse HEAD)
dirty=$([ -n "$(git status --porcelain --untracked-files=no)" ] && echo true || echo false)
epoch=$(git log -1 --format=%ct)
export SOURCE_DATE_EPOCH=$epoch

# Pinned tools, verified on every use.
fetch() { # url sha256 path
    if [ ! -f "$3" ]; then
        mkdir -p "$(dirname "$3")"
        curl -fsSL --retry 3 -o "$3.part" "$1"
        mv "$3.part" "$3"
    fi
    echo "$2  $3" | sha256sum -c --quiet - || { echo "VERDICT fail: hash mismatch $3" >&2; exit 4; }
}
appimagetool="$tools/appimagetool-$APPIMAGETOOL_VERSION-x86_64.AppImage"
runtime="$tools/runtime-x86_64-$RUNTIME_VERSION"
fetch "$APPIMAGETOOL_URL" "$APPIMAGETOOL_SHA256" "$appimagetool"
fetch "$RUNTIME_URL" "$RUNTIME_SHA256" "$runtime"
# A tool cache on a Windows drive may lose the executable bit; run a private copy.
work=$(mktemp -d "${TMPDIR:-/tmp}/drig-package.XXXXXX")
trap 'rm -rf "$work"' EXIT
cp "$appimagetool" "$work/appimagetool"
chmod +x "$work/appimagetool"

if [ "$build" = 1 ]; then
    # shellcheck disable=SC2086
    cargo build --release --locked -p grid-app --bin grid-app $cargo_args
fi
target_dir=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
binary="$target_dir/release/grid-app"
file -b "$binary" | grep -q "ELF 64-bit LSB pie executable, x86-64" || {
    echo "VERDICT fail: $binary is not an x86-64 ELF executable" >&2; exit 5; }
# Required glibc: the newest version needed without the WEAK flag (std references
# pidfd_spawnp@GLIBC_2.39 weakly and falls back when the loader lacks it).
glibc=$(readelf -V "$binary" | awk '/Name: GLIBC_/ && /Flags: none/ {print $3}' \
    | sort -t. -k1,1 -k2,2n -k3,3n -u | tail -1)

name="DRIG-v$version-linux-x86_64"
stage="$work/$name"
mkdir -p "$stage"
install -m 755 "$binary" "$stage/drig"
install -m 644 LICENSE "$stage/LICENSE"
cp -r LICENSES "$stage/LICENSES"
install -m 644 crates/grid-app/assets/fonts/FiraMono-LICENSE "$stage/LICENSES/FiraMono-OFL-1.1.txt"
cat > "$stage/build.json" <<EOF
{
  "version": "$version",
  "source_revision": "$revision",
  "source_dirty": $dirty,
  "target": "x86_64-unknown-linux-gnu",
  "rustc": "$(rustc -V)",
  "min_glibc": "${glibc#GLIBC_}",
  "bundled_game_data": false
}
EOF
cat > "$stage/README.txt" <<EOF
DRIG $version for Linux x86_64 (source $revision)

DRIG is an independent open-source engine for Race Driver: GRID (2008). It contains no game
files: it reads them from your own purchased Windows installation of GRID.

Start:      ./drig            (or the .AppImage; chmod +x it first)
Help:       ./drig --help
Game files: found in the Steam libraries (~/.steam/steam, ~/.local/share/Steam and the Flatpak
            Steam, plus every library in steamapps/libraryfolders.vdf), or pass
            --game-root /path/to/Grid (the folder with grid.exe and cars/).
            File names are matched ignoring case, so a copy from Windows works as it is.
Data:       settings, records, replays and logs stay next to the program (portable); the
            AppImage keeps them next to the .AppImage file. --data-dir DIR overrides this.

Needs:      x86_64, glibc ${glibc#GLIBC_} or newer, a Vulkan driver (Mesa or the GPU vendor's),
            X11 or Wayland, ALSA (PulseAudio and PipeWire provide it). The AppImage needs
            FUSE 2 (libfuse2) or: ./DRIG-v$version-linux-x86_64.AppImage --appimage-extract-and-run

License: GPL-3.0-or-later (LICENSE); third-party notices in LICENSES/.
Source: https://github.com/PipeKnight/drig-race-driver
EOF

mkdir -p "$out"
out=$(cd "$out" && pwd)
tarball="$out/$name.tar.gz"
appimage="$out/$name.AppImage"
rm -f "$tarball" "$appimage"
find "$stage" -exec touch -h -d "@$epoch" {} +
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$epoch" -C "$work" -cf - "$name" \
    | gzip -n -9 > "$tarball"

# AppImage: the same files under usr/, a desktop entry and the icon at the AppDir root.
appdir="$work/AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/doc/drig"
install -m 755 "$stage/drig" "$appdir/usr/bin/drig"
cp -r "$stage/LICENSE" "$stage/LICENSES" "$stage/README.txt" "$stage/build.json" "$appdir/usr/share/doc/drig/"
install -m 644 crates/grid-app/assets/macos/DRIG.png "$appdir/drig.png"
cat > "$appdir/drig.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=DRIG
Comment=Open-source engine for Race Driver: GRID (2008)
Exec=drig
Icon=drig
Categories=Game;
Terminal=false
X-AppImage-Version=$version
EOF
# The runtime sets APPIMAGE to the .AppImage file; the engine keeps its portable
# data folder next to that file (docs/LINUX.md "Data folder").
cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
here=$(dirname "$(readlink -f "$0")")
exec "$here/usr/bin/drig" "$@"
EOF
chmod 755 "$appdir/AppRun"
find "$appdir" -exec touch -h -d "@$epoch" {} +
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$work/appimagetool" --no-appstream \
    --runtime-file "$runtime" "$appdir" "$appimage" > "$work/appimagetool.log" 2>&1 || {
    cat "$work/appimagetool.log" >&2; echo "VERDICT fail: appimagetool" >&2; exit 6; }

sums="$out/SHA256SUMS.txt"
touch "$sums"
grep -v -F -e "  $name.tar.gz" -e "  $name.AppImage" "$sums" > "$work/sums" || true
(cd "$out" && sha256sum "$name.tar.gz" "$name.AppImage") >> "$work/sums"
sort -k2 "$work/sums" > "$sums"
echo "built $revision dirty=$dirty min_glibc=${glibc#GLIBC_} appimagetool=$APPIMAGETOOL_VERSION runtime=$RUNTIME_VERSION"
grep -F -e "$name.tar.gz" -e "$name.AppImage" "$sums"
ls -l "$tarball" "$appimage" | awk '{print $5, $NF}'
echo "VERDICT pass out=$out"
