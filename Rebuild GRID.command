#!/bin/bash
# Double-click to rebuild fresh main as a private macOS DMG.
set -u
repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd) || exit 1
python3 -I "$repo_dir/tools/rebuild_macos.py" "$@"
result=$?
if [ -t 0 ]; then
  echo
  read -r -p "Нажми Enter, чтобы закрыть окно. " _
fi
exit "$result"
