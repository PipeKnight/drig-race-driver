"""Rebuild fresh origin/main as a private macOS DMG without changing the current checkout.

See docs/MACOS.md. Default: embed the owner's preserved game files, then clean
temporary sources, app staging and Cargo artifacts. --code-only omits game data.
"""

import argparse
from datetime import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
GIB = 1024 ** 3
MARKERS = ("grid.exe", "cars/lac/lac.ned", "database/schema.bin", "database/database.bin")


def game_folder(explicit=None):
    if explicit is not None:
        candidates = [Path(explicit).expanduser()]
    else:
        candidates = [ROOT / "local/macos-portable/source/Grid.reference",
                      ROOT / "local/macos-portable/source/Grid"]
        saved = Path.home() / "Library/Application Support/GRID Reassembly/game-root.txt"
        if saved.is_file():
            candidates.append(Path(saved.read_text().strip()))
    for path in candidates:
        if all((path / marker).is_file() for marker in MARKERS):
            return path.resolve()
    raise RuntimeError("Не найдены полные файлы GRID. Укажи папку через --game-root PATH.")


def checksum(path):
    h = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


class Runner:
    def __init__(self, log=None):
        self.log = log

    def run(self, args, cwd=None, env=None):
        args = [str(a) for a in args]
        print("→ " + " ".join(args), flush=True)
        with subprocess.Popen(args, cwd=cwd or ROOT, env=env, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, text=True, errors="replace") as process:
            for line in process.stdout:
                print(line, end="", flush=True)
                if self.log:
                    self.log.write(line)
                    self.log.flush()
            code = process.wait()
        if code:
            raise subprocess.CalledProcessError(code, args)


def output(args, cwd=None):
    return subprocess.check_output([str(a) for a in args], cwd=cwd or ROOT, text=True).strip()


def rust_commands(channel):
    """Use the installed matching compiler or an already installed rustup toolchain."""
    version = output(["rustc", "--version"]).split()[1]
    if channel != version:
        if not shutil.which("rustup"):
            raise RuntimeError(f"Нужен Rust {channel}; установлен {version}.")
        prefix = ["rustup", "run", channel]
        try:
            output([*prefix, "rustc", "--version"])
        except subprocess.CalledProcessError as error:
            raise RuntimeError(f"Установи нужный Rust: rustup toolchain install {channel} --profile minimal") from error
    else:
        prefix = []
    host = next(line.split(": ", 1)[1] for line in
                output([*prefix, "rustc", "-vV"]).splitlines() if line.startswith("host: "))
    if not host.endswith("-apple-darwin"):
        raise RuntimeError(f"Нужен нативный Rust для macOS, сейчас: {host}")
    if host.split("-")[0] != {"arm64": "aarch64", "x86_64": "x86_64"}.get(platform.machine()):
        raise RuntimeError("Архитектура Rust не совпадает с архитектурой этого Mac.")
    return [*prefix, "cargo"], host


def build(args, runner):
    game = None if args.code_only else game_folder(args.game_root)
    ref = "refs/codex/macos-rebuild/main"
    runner.run(["git", "fetch", "--no-tags", "origin", f"+refs/heads/main:{ref}"])
    revision = output(["git", "rev-parse", f"{ref}^{{commit}}"])
    toolchain = output(["git", "show", f"{revision}:rust-toolchain.toml"])
    channel = re.search(r'^\s*channel\s*=\s*"([^"]+)"', toolchain, re.M)
    if not channel:
        raise RuntimeError("Не удалось прочитать версию Rust из свежей main.")
    cargo, host = rust_commands(channel.group(1))
    game_bytes = sum(p.stat().st_size for p in game.rglob("*") if p.is_file()) if game else 0
    required = 4 * GIB + 2 * game_bytes
    free = shutil.disk_usage(ROOT).free
    if free < required:
        raise RuntimeError(f"Недостаточно места: свободно {free / GIB:.1f} GiB, нужно примерно {required / GIB:.1f} GiB.")
    print(f"\nСвежая main: {revision}\nMac: {host}\nФайлы GRID: {game or 'не включаются'}", flush=True)
    if args.dry_run:
        print("Проверка плана завершена. Компиляция и упаковка не запускались.")
        return
    stamp = datetime.now().astimezone().strftime("%Y%m%d-%H%M%S")
    destination = ROOT / "local/macos-rebuilds" / f"{stamp}-{revision[:8]}-{uuid.uuid4().hex[:6]}"
    work = destination / "work"
    source = work / "source"
    target = ROOT / "local/macos-build-cache/target" if args.keep_cache else work / "target"
    stage = work / "stage"
    name = f"{'DRIG' if args.code_only else 'GRID'}-macos-{platform.machine()}-{revision[:8]}.dmg"
    dmg = destination / name
    destination.mkdir(parents=True)
    manifest = {"source_revision": revision, "rust_channel": channel.group(1), "target": host,
                "bundled_game_data": bool(game), "game_root": str(game) if game else None,
                "keep_cache": args.keep_cache, "status": "building"}
    manifest_path = destination / "build.json"
    try:
        with (destination / "build.log").open("w", encoding="utf-8") as log:
            job = Runner(log)
            work.mkdir()
            # A separate detached clone, not a checkout/reset/stash of the user's branch.
            job.run(["git", "clone", "--shared", "--no-checkout", "--local", ROOT, source])
            job.run(["git", "checkout", "--detach", revision], cwd=source)
            env = dict(os.environ, CARGO_TARGET_DIR=str(target), GRID_BUILD_REVISION=revision)
            env["CARGO_BUILD_BUILD_DIR"] = str(target / "build")
            env.pop("CARGO_BUILD_TARGET", None)
            job.run([*cargo, "build", "--locked", "--release", "--target", host,
                     "-p", "grid-app", "--bin", "grid-app"], cwd=source, env=env)
            binary = target / host / "release/grid-app"
            job.run([binary, "--help"], cwd=source, env=env)
            manifest["engine_sha256"] = checksum(binary)
            command = [sys.executable, "-I", ROOT / "tools/build_macos.py", "--no-build",
                       "--binary", binary, "--source-root", source,
                       "--output-dir", stage, "--dmg", dmg]
            if game:
                command += ["--game-root", game]
            job.run(command, env=env)
            metadata = json.loads((stage / "DRIG.app/Contents/Resources/build.json").read_text())
            if metadata["source_revision"] != revision or metadata["bundled_game_data"] != bool(game):
                raise RuntimeError("Версия или состав готового приложения не совпадают с планом.")
            manifest.update(status="complete", dmg=name, dmg_bytes=dmg.stat().st_size,
                            dmg_sha256=checksum(dmg))
            (destination / (name + ".sha256")).write_text(manifest["dmg_sha256"] + "  " + name + "\n")
    except BaseException as error:
        manifest.update(status="failed", error=str(error))
        if dmg.exists():
            dmg.unlink()  # Only this run's incomplete output, never an older build.
        raise
    finally:
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        if work.exists():
            shutil.rmtree(work)
        print(f"\nЛог и сведения о сборке: {destination}", flush=True)
    print(f"\nГОТОВО: {dmg}\nОткрой DMG и скопируй DRIG.app в удобную папку.", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-root", type=Path, help="folder of your purchased Windows GRID 2008")
    parser.add_argument("--code-only", action="store_true", help="omit original game resources")
    parser.add_argument("--keep-cache", action="store_true", help="retain Cargo cache for faster future builds")
    parser.add_argument("--dry-run", action="store_true", help="fetch main and check the plan without building")
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Запускай этот скрипт на Mac.")
    if args.code_only and args.game_root:
        parser.error("--code-only и --game-root нельзя использовать вместе")
    for tool in ["git", "cargo", "rustc", "codesign", "hdiutil", "lipo"]:
        if not shutil.which(tool):
            parser.error(f"Не найден инструмент {tool}. Установи Rust и Xcode Command Line Tools.")
    lock = ROOT / "local/macos-rebuild.lock"
    lock.parent.mkdir(exist_ok=True)
    with lock.open("a") as handle:
        try:
            fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            parser.error("Пересборка уже запущена. Дождись её завершения.")
        try:
            build(args, Runner())
        except (RuntimeError, subprocess.CalledProcessError, OSError) as error:
            print(f"\nСборка не завершена: {error}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
