"""Exercise rebuild isolation and cleanup with real Git repos and a simulated compiler."""

import argparse
import fcntl
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("rebuild_macos", Path(__file__).parents[1] / "rebuild_macos.py")
MOD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MOD)


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True, stderr=subprocess.DEVNULL).strip()


class RebuildTests(unittest.TestCase):
    def scenario(self, fail=False, keep_cache=False):
        with tempfile.TemporaryDirectory(prefix="GRID rebuild test ") as directory:
            root = Path(directory) / "working tree"
            root.mkdir()
            git(root, "init", "-b", "main")
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.0"\n')
            (root / "tracked.txt").write_text("remote main\n")
            git(root, "add", ".")
            git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-m", "fixture")
            remote = Path(directory) / "remote.git"
            subprocess.run(["git", "clone", "--bare", str(root), str(remote)], check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            git(root, "remote", "add", "origin", str(remote))
            revision = git(root, "rev-parse", "HEAD")
            git(root, "switch", "-c", "local-work")
            (root / "tracked.txt").write_text("uncommitted owner changes\n")
            (root / "untracked.txt").write_text("also preserve\n")
            older = root / "local/macos-rebuilds/older/good.dmg"
            older.parent.mkdir(parents=True)
            older.write_bytes(b"older successful output")
            real_runner = MOD.Runner

            class CompilerFixture(real_runner):
                def run(self, args, cwd=None, env=None):
                    args = [str(x) for x in args]
                    if args[0] == "fixture-cargo":
                        self_outer.assertEqual(git(cwd, "rev-parse", "HEAD"), revision)
                        self_outer.assertEqual((Path(cwd) / "tracked.txt").read_text(), "remote main\n")
                        if fail:
                            raise subprocess.CalledProcessError(9, args)
                        binary = Path(env["CARGO_TARGET_DIR"]) / "aarch64-apple-darwin/release/grid-app"
                        binary.parent.mkdir(parents=True)
                        binary.write_text("#!/bin/sh\nexit 0\n")
                        binary.chmod(0o755)
                    elif "--binary" in args:
                        stage = Path(args[args.index("--output-dir") + 1])
                        metadata = stage / "DRIG.app/Contents/Resources/build.json"
                        metadata.parent.mkdir(parents=True)
                        metadata.write_text(json.dumps({"source_revision": revision, "bundled_game_data": False}))
                        Path(args[args.index("--dmg") + 1]).write_bytes(b"simulated verified image")
                    else:
                        return super().run(args, cwd=cwd or root, env=env)

            self_outer = self
            args = argparse.Namespace(code_only=True, game_root=None, dry_run=False, keep_cache=keep_cache)
            with patch.object(MOD, "ROOT", root), patch.object(MOD, "Runner", CompilerFixture), \
                 patch.object(MOD, "rust_commands", return_value=(["fixture-cargo"], "aarch64-apple-darwin")):
                if fail:
                    with self.assertRaises(subprocess.CalledProcessError):
                        MOD.build(args, CompilerFixture())
                else:
                    MOD.build(args, CompilerFixture())
            self.assertEqual(git(root, "branch", "--show-current"), "local-work")
            self.assertEqual((root / "tracked.txt").read_text(), "uncommitted owner changes\n")
            self.assertEqual((root / "untracked.txt").read_text(), "also preserve\n")
            self.assertEqual(older.read_bytes(), b"older successful output")
            manifests = list((root / "local/macos-rebuilds").glob("*/build.json"))
            self.assertEqual(len(manifests), 1)
            result = json.loads(manifests[0].read_text())
            self.assertEqual(result["status"], "failed" if fail else "complete")
            self.assertEqual(result["source_revision"], revision)
            self.assertFalse((manifests[0].parent / "work").exists())
            if not fail:
                self.assertTrue((manifests[0].parent / result["dmg"]).is_file())
                self.assertEqual((root / "local/macos-build-cache/target").exists(), keep_cache)

    def test_success_preserves_dirty_checkout_and_cleans_temporary_build(self):
        self.scenario()

    def test_failed_compile_preserves_checkout_and_previous_build(self):
        self.scenario(fail=True)

    def test_explicit_cache_is_retained_without_retaining_source_snapshot(self):
        self.scenario(keep_cache=True)

    def test_second_invocation_is_rejected_before_fetch_or_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "local").mkdir()
            with (root / "local/macos-rebuild.lock").open("a") as lock:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                with patch.object(MOD, "ROOT", root), patch.object(MOD.sys, "argv", ["rebuild_macos.py"]), \
                     patch.object(MOD.shutil, "which", return_value="/usr/bin/test-tool"), \
                     patch.object(MOD, "build") as build:
                    with self.assertRaises(SystemExit) as error:
                        MOD.main()
                    self.assertEqual(error.exception.code, 2)
                    build.assert_not_called()


if __name__ == "__main__":
    unittest.main()
