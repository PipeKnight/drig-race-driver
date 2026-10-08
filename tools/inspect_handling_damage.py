"""Read-only installed handling-damage archive survey; evidence stays private."""
import argparse
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from validate_binxml import entries, decode

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    baseline = json.loads((ROOT/'local/reference-race/load-20261006-0128/steam-resource-verification.json').read_text())
    records = [r for r in baseline['resources'] if Path(r['relative_path']).name == 'raceload.jpk']
    assert len(records) == 1
    record = records[0]
    source = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')/record['relative_path']
    data = source.read_bytes()
    sha = hashlib.sha256(data).hexdigest()
    assert sha == record['steam_sha256']
    reports = []
    for name, payload, raw in entries(data):
        if name not in ['standard.hdm', 'openwheeler.hdm', 'hdm_loader.xml']: continue
        assert raw
        target = out/name
        target.write_bytes(payload)
        if payload.startswith(bytes.fromhex('1a225272')):
            root = decode(payload)[1]
            format_name = 'binary XML'
        else:
            # GRID HDM uses legacy <!description> blocks, which strict XML
            # parsers reject. Bound this adapter to plain ASCII descriptions.
            declarations = re.findall(rb'<![^>]*>', payload)
            assert declarations and all(not d.startswith((b'<!DOCTYPE',b'<!ENTITY',b'<![')) for d in declarations)
            cleaned = re.sub(rb'<![^>]*>', b'', payload)
            root = ET.fromstring(cleaned)
            format_name = 'text XML with legacy description declarations'
        tree = ET.ElementTree(root)
        ET.indent(tree)
        tree.write(out/(name+'.decoded.xml'),encoding='utf-8',xml_declaration=True)
        reports.append(dict(entry=name,sha256=hashlib.sha256(payload).hexdigest(),
                            format=format_name,root=root.tag,elements=sum(1 for _ in root.iter()),
                            nodes=[dict(tag=e.tag,attributes=e.attrib) for e in root.iter()]))
    assert len(reports) == 3
    (out/'survey.json').write_text(json.dumps(dict(source=str(source),archive_sha256=sha,
                                                reports=reports, original_loader_executed=False),indent=2))
    print(json.dumps([dict(entry=r['entry'],format=r['format'],root=r['root'],elements=r['elements']) for r in reports]))


if __name__ == '__main__':
    main()
