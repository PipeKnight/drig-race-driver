"""Read-only D3D9 shader disassembly using Windows' installed Microsoft compiler."""
import argparse
import ctypes as C
import hashlib
import json
from pathlib import Path


def disassemble(data):
    if not 8 <= len(data) <= 1024 * 1024 or data[:4] not in (b'\x00\x03\xff\xff', b'\x00\x03\xfe\xff'):
        raise ValueError('Expected bounded D3D9 shader model 3 bytecode')
    dll = C.WinDLL(r'C:\Windows\System32\d3dcompiler_47.dll')
    fn = dll.D3DDisassemble
    fn.argtypes = [C.c_void_p, C.c_size_t, C.c_uint, C.c_char_p, C.POINTER(C.c_void_p)]
    fn.restype = C.c_long
    blob = C.c_void_p()
    buffer = C.create_string_buffer(data)
    result = fn(buffer, len(data), 0, None, C.byref(blob))
    if result < 0 or not blob:
        raise RuntimeError(f'D3DDisassemble failed: 0x{result & 0xffffffff:08x}')
    vtable = C.cast(blob, C.POINTER(C.POINTER(C.c_void_p))).contents
    release = C.WINFUNCTYPE(C.c_ulong, C.c_void_p)(vtable[2])
    try:
        pointer = C.WINFUNCTYPE(C.c_void_p, C.c_void_p)(vtable[3])(blob)
        size = C.WINFUNCTYPE(C.c_size_t, C.c_void_p)(vtable[4])(blob)
        if size > 16 * 1024 * 1024:
            raise ValueError('Disassembly exceeds bound')
        return C.string_at(pointer, size).rstrip(b'\0')
    finally:
        release(blob)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parent.parent / 'local').resolve()
    output = args.output.resolve()
    if not output.is_relative_to(private):
        parser.error('Original shader disassembly must remain under ignored local/')
    if output.exists():
        raise FileExistsError(output)
    with args.source.open('rb') as stream:
        data = stream.read(1024 * 1024 + 1)
    assembly = disassemble(data)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('xb') as stream:
        stream.write(assembly)
    print(json.dumps(dict(source=str(args.source), bytes=len(data),
                          source_sha256=hashlib.sha256(data).hexdigest(),
                          output_sha256=hashlib.sha256(assembly).hexdigest())))


if __name__ == '__main__':
    main()
