"""Verify preserved reference hashes and saved compatibility experiment results."""
import argparse
import hashlib
import json
from pathlib import Path


def sha256(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--settings', type=Path, required=True)
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    reference = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
    out = project / 'local' / 'compatibility'
    baseline = json.loads((project / 'local' / 'phase0' / 'installation.json').read_text(encoding='utf-8'))
    changed = [item['path'] for item in baseline['selected_hashes'] if sha256(reference / item['path']) != item['sha256']]
    backup = out / 'legacy-confirmation-settings-before'
    settings_checks = {p.name: (args.settings / p.name).exists() and sha256(args.settings / p.name) == sha256(p) for p in backup.glob('*.xml')}
    runs = {name: json.loads((out / f'{name}.json').read_text(encoding='utf-8-sig')) for name in ['unpacked', 'laa', 'vram', 'mtfix-retry', 'legacy', 'utf8-manifest', 'legacy-confirmation']}
    pair = json.loads((out / 'manifest-binary-comparison.json').read_text(encoding='utf-8'))
    record = {'OriginalSelectedFilesChecked': len(baseline['selected_hashes']), 'OriginalChangedFiles': changed, 'SettingsByteExact': settings_checks, 'PairedFilesOnlyCodepageTextDiffers': pair['OnlyCodepageTextDiffers'], 'AllCompletedRunsRestoredSettings': all(r['ExperimentCompleted'] and r['SettingsRestored'] and r['ReferenceExeUnchanged'] for r in runs.values()), 'WorkingCopySHA256': sha256(Path(r'C:\Users\debros\Tools\grid-research\grid-compat\grid.exe')), 'MenuVerification': 'Owner confirms main menu reached during legacy-codepage confirmation; see owner-observation.json'}
    (out / 'verification.json').write_text(json.dumps(record, indent=2), encoding='utf-8')
    print(json.dumps(record, indent=2))
    if changed or not settings_checks or not all(settings_checks.values()) or not record['PairedFilesOnlyCodepageTextDiffers'] or not record['AllCompletedRunsRestoredSettings']:
        raise SystemExit('Verification failed')


if __name__ == '__main__':
    main()
