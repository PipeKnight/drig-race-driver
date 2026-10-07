"""Run the native inspectors against locally purchased files; keep output private."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("installation", type=Path)
    parser.add_argument("--inspector", type=Path, default=Path("target/debug/grid-tools.exe"))
    parser.add_argument("--out", type=Path, default=Path("local/formats/coverage.json"))
    args = parser.parse_args()
    inspector = args.inspector.resolve(strict=True)
    root = args.installation.resolve(strict=True)
    files = sorted((root / "cars/lac").rglob("*.pssg"))
    files += sorted(root.rglob("*.jpk"))
    results = []
    for path in files:
        result = subprocess.run(
            [str(inspector), path.suffix[1:], str(path)],
            capture_output=True, text=True, encoding="utf-8", timeout=30,
        )
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        results.append({
            "path": path.relative_to(root).as_posix(),
            "sha256": digest,
            "exit_code": result.returncode,
            "summary": result.stdout.splitlines()[0] if result.stdout else "",
            "error": result.stderr.strip(),
        })
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    failures = [r for r in results if r["exit_code"] != 0]
    print(json.dumps({"checked": len(results), "passed": len(results)-len(failures),
                      "failures": failures}, indent=2))
    return bool(failures)


if __name__ == "__main__":
    raise SystemExit(main())
