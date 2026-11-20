"""Install the repository's git hooks once per clone (docs/TOOLCHAIN.md "Git hooks").

    py -I tools/hooks/install.py              # git config core.hooksPath .githooks
    py -I tools/hooks/install.py --uninstall  # remove that setting

Sets core.hooksPath in the clone's shared config, so every worktree of the clone uses the hooks
in its own checked-out .githooks/ (pre-commit and commit-msg, which call
tools/hooks/check_commit.py). Git does not install hooks by itself for security reasons, so each
new clone runs this once. Python standard library only.
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)


def main():
    current = git("config", "--get", "core.hooksPath").stdout.strip()
    if "--uninstall" in sys.argv[1:]:
        if current == ".githooks":
            git("config", "--unset", "core.hooksPath")
            print("hooks: uninstalled (core.hooksPath removed)")
        else:
            print("hooks: not installed by this tool (core.hooksPath=%r); nothing changed" % current)
        return 0
    if current and current != ".githooks":
        print("hooks: core.hooksPath is already %r; not overwritten. Remove it first if the DRIG hooks "
              "should run: git config --unset core.hooksPath" % current)
        return 1
    if not (ROOT / ".githooks" / "pre-commit").is_file():
        print("hooks: .githooks/pre-commit missing in %s" % ROOT)
        return 1
    r = git("config", "core.hooksPath", ".githooks")
    if r.returncode != 0:
        print("hooks: git config failed: %s" % r.stderr.strip())
        return 1
    print("hooks: installed (core.hooksPath=.githooks): pre-commit and commit-msg run "
          "tools/hooks/check_commit.py on every commit")
    return 0


if __name__ == "__main__":
    sys.exit(main())
