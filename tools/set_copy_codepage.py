"""Embed a per-process codepage request in the private GRID compatibility copy."""
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--codepage', choices=['en-US', 'uk-UA', 'UTF-8'], required=True)
    parser.add_argument('--out-dir', type=Path, help='Separate evidence directory for later experiments')
    args = parser.parse_args()
    target = Path(r'C:\Users\debros\Tools\grid-research\grid-compat\grid.exe')
    if target.resolve() != target or target.is_symlink():
        raise RuntimeError('Refusing a redirected target')
    out = args.out_dir or Path(__file__).resolve().parent.parent / 'local' / 'compatibility'
    out.mkdir(parents=True, exist_ok=True)
    if (out / ('manifest-' + args.codepage + '.json')).exists():
        raise RuntimeError('Manifest experiment already recorded; select a fresh --out-dir')
    kernel = c.WinDLL('kernel32', use_last_error=True)
    kernel.LoadLibraryExW.argtypes = [w.LPCWSTR, w.HANDLE, w.DWORD]
    kernel.LoadLibraryExW.restype = w.HMODULE
    kernel.FindResourceW.argtypes = [w.HMODULE, c.c_void_p, c.c_void_p]
    kernel.FindResourceW.restype = c.c_void_p
    kernel.SizeofResource.argtypes = [w.HMODULE, c.c_void_p]
    kernel.SizeofResource.restype = w.DWORD
    kernel.LoadResource.argtypes = [w.HMODULE, c.c_void_p]
    kernel.LoadResource.restype = c.c_void_p
    kernel.LockResource.argtypes = [c.c_void_p]
    kernel.LockResource.restype = c.c_void_p
    kernel.FreeLibrary.argtypes = [w.HMODULE]
    kernel.BeginUpdateResourceW.argtypes = [w.LPCWSTR, w.BOOL]
    kernel.BeginUpdateResourceW.restype = w.HANDLE
    kernel.UpdateResourceW.argtypes = [w.HANDLE, c.c_void_p, c.c_void_p, w.WORD, c.c_void_p, w.DWORD]
    kernel.UpdateResourceW.restype = w.BOOL
    kernel.EndUpdateResourceW.argtypes = [w.HANDLE, w.BOOL]
    kernel.EndUpdateResourceW.restype = w.BOOL
    module = kernel.LoadLibraryExW(str(target), None, 2)
    if not module:
        raise c.WinError(c.get_last_error())
    existing = None
    try:
        resource = kernel.FindResourceW(module, c.c_void_p(1), c.c_void_p(24))
        if resource:
            size = kernel.SizeofResource(module, resource)
            ptr = kernel.LockResource(kernel.LoadResource(module, resource))
            existing = c.string_at(ptr, size).rstrip(b'\0')
    finally:
        kernel.FreeLibrary(module)
    v1 = 'urn:schemas-microsoft-com:asm.v1'
    v3 = 'urn:schemas-microsoft-com:asm.v3'
    ws = 'http://schemas.microsoft.com/SMI/2019/WindowsSettings'
    ET.register_namespace('', v1)
    ET.register_namespace('asmv3', v3)
    ET.register_namespace('ws2019', ws)
    if existing:
        (out / 'original-copy-manifest.xml').write_bytes(existing)
        root = ET.fromstring(existing)
    else:
        root = ET.Element(f'{{{v1}}}assembly', {'manifestVersion': '1.0'})
        ET.SubElement(root, f'{{{v1}}}assemblyIdentity', {'type': 'win32', 'name': 'Grid.Compatibility.Research', 'version': '1.0.0.0', 'processorArchitecture': 'x86'})
    application = root.find(f'{{{v3}}}application')
    if application is None:
        application = ET.SubElement(root, f'{{{v3}}}application')
    settings = application.find(f'{{{v3}}}windowsSettings')
    if settings is None:
        settings = ET.SubElement(application, f'{{{v3}}}windowsSettings')
    active = settings.find(f'{{{ws}}}activeCodePage')
    if active is None:
        active = ET.SubElement(settings, f'{{{ws}}}activeCodePage')
    active.text = args.codepage
    manifest = ET.tostring(root, encoding='utf-8', xml_declaration=True)
    before = hashlib.sha256(target.read_bytes()).hexdigest()
    update = kernel.BeginUpdateResourceW(str(target), False)
    if not update:
        raise c.WinError(c.get_last_error())
    buffer = c.create_string_buffer(manifest)
    if not kernel.UpdateResourceW(update, c.c_void_p(24), c.c_void_p(1), 0, buffer, len(manifest)):
        error = c.get_last_error()
        kernel.EndUpdateResourceW(update, True)
        raise c.WinError(error)
    if not kernel.EndUpdateResourceW(update, False):
        raise c.WinError(c.get_last_error())
    (out / ('manifest-' + args.codepage + '.xml')).write_bytes(manifest)
    record = {'Target': str(target), 'RequestedProcessCodepage': args.codepage, 'HadEmbeddedManifest': existing is not None, 'SHA256Before': before, 'SHA256After': hashlib.sha256(target.read_bytes()).hexdigest(), 'SystemSettingsChanged': False}
    (out / ('manifest-' + args.codepage + '.json')).write_text(json.dumps(record, indent=2), encoding='utf-8')
    print(json.dumps(record))


if __name__ == '__main__':
    main()
