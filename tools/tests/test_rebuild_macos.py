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
    def test_same_second_launches_keep_distinct_logs(self):
        spec = importlib.util.spec_from_file_location(
            "build_macos", Path(__file__).parents[1] / "build_macos.py")
        packager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(packager)
        with tempfile.TemporaryDirectory(prefix="DRIG log test ") as directory:
            root = Path(directory)
            macos = root / "DRIG.app/Contents/MacOS"
            macos.mkdir(parents=True)
            (macos.parent / "Resources/Grid").mkdir(parents=True)
            launcher = macos / "DRIG"
            launcher.write_text(packager.LAUNCHER.replace("@BUILD_REVISION@", "1ed1b05002e4"))
            clock = root / "date"
            clock.write_text("#!/bin/sh\necho 20261125T000100Z\n")
            clock.chmod(0o755)
            engine = macos / "grid-app"
            engine.write_text("#!/bin/sh\nprintf 'race output\\n'\n")
            engine.chmod(0o755)
            data = root / "DRIG-data"
            data.mkdir()
            previous = data / "drig.log"
            previous.write_text("earlier session")

            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ["PATH"])
            env.pop("GRID_DATA_DIR", None)
            subprocess.run(["sh", launcher], env=env, check=True)
            [first] = (data / "playtest").iterdir()
            subprocess.run(["sh", launcher], env=env, check=True)
            logs = list((data / "playtest").glob("drig-20261125T000100Z-1ed1b05002e4.*"))
            self.assertEqual(len(logs), 2)
            self.assertIn(first, logs)
            self.assertTrue(all(p.read_text() == "race output\n" for p in logs))
            self.assertEqual(previous.read_text(), "earlier session")

    def test_installed_resources_are_preferred_and_explicit_path_overrides(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            installed = root / "Applications/DRIG.app/Contents/Resources/Grid"
            fallback = root / "local/macos-portable/source/Grid.reference"
            for game in (installed, fallback):
                for marker in MOD.MARKERS:
                    path = game / marker
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.touch()

            with patch.object(MOD, "ROOT", root), patch.object(MOD, "INSTALLED_GAME_ROOT", installed):
                self.assertEqual(MOD.game_folder(), installed.resolve())
                self.assertEqual(MOD.game_folder(fallback), fallback.resolve())
                (installed / "grid.exe").unlink()
                self.assertEqual(MOD.game_folder(), fallback.resolve())

    def scenario(self, fail=False, keep_cache=False, release_tag=None):
        with tempfile.TemporaryDirectory(prefix="GRID rebuild test ") as directory:
            root = Path(directory) / "working tree"
            root.mkdir()
            git(root, "init", "-b", "main")
            (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.0"\n')
            (root / "tracked.txt").write_text("remote main\n")
            git(root, "add", ".")
            git(root, "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-m", "fixture")
            if release_tag:
                git(root, "tag", release_tag)
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
                        self_outer.assertIn("--no-default-features", args)
                        self_outer.assertEqual(git(cwd, "rev-parse", "HEAD"), revision)
                        self_outer.assertEqual((Path(cwd) / "tracked.txt").read_text(), "remote main\n")
                        if fail:
                            raise subprocess.CalledProcessError(9, args)
                        binary = Path(env["CARGO_TARGET_DIR"]) / "aarch64-apple-darwin/release/grid-app"
                        binary.parent.mkdir(parents=True)
                        binary.write_text(f"#!/bin/sh\nprintf '%s\\n' 'DRIG 0.1.0 (revision {revision[:12]})'\n")
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
            args = argparse.Namespace(code_only=True, game_root=None, dry_run=False, release_tag=release_tag, keep_cache=keep_cache)
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
                if release_tag:
                    self.assertTrue(result["dmg"].startswith(f"DRIG-{release_tag}-macos-"))
                self.assertEqual((root / "local/macos-build-cache/target").exists(), keep_cache)

    def test_success_preserves_dirty_checkout_and_cleans_temporary_build(self):
        self.scenario()

    def test_tagged_player_build_names_image_after_release(self):
        self.scenario(release_tag="v0.1.0")

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
