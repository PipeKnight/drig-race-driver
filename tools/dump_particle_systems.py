"""Write readable private copies of GRID particle-system binary XML files.

Usage: python -I tools/dump_particle_systems.py GAME_ROOT OUTPUT_DIR [NAME ...]

OUTPUT_DIR must lie beneath a local/ directory; the copies are derived game
data and must not be published.
"""
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from validate_binxml import decode  # noqa: E402


def main():
    root, out = Path(sys.argv[1]), Path(sys.argv[2]).resolve()
    if 'local' not in out.parts:
        raise SystemExit('output must stay beneath local/')
    out.mkdir(parents=True, exist_ok=True)
    folder = root/'effects'/'particle_systems'
    names = sys.argv[3:] or sorted(p.stem for p in folder.glob('*.xml'))
    for name in names:
        data = (folder/f'{name}.xml').read_bytes()
        if data.startswith(bytes.fromhex('1a225272')):
            _, tree, _ = decode(data)
            ET.indent(tree)
            text = ET.tostring(tree, encoding='unicode')
        else:
            text = data.decode('utf-8-sig')
        (out/f'{name}.xml').write_text(text, encoding='utf-8')
        print(name, len(text))


if __name__ == '__main__':
    main()
