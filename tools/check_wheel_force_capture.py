"""Owned-memory decoder/ownership checks; does not attach to or control a game."""
import json
from pathlib import Path
import struct
import capture_reference_spawn as capture


def main():
    sp, body, view, solver, material = 0x300000, 0x500000, 0x700000, 0x710000, 0x720000

    def fixture():
        stack = bytearray(struct.pack('<4I', 0xa04182, body, 0x3c888889, 0))
        data = bytearray(0x2600)
        struct.pack_into('<I', data, 0, view)
        struct.pack_into('<I', data, 0x1620, 4)
        struct.pack_into('<I', data, 0x18f0, 2)
        struct.pack_into('<2I', data, 0x18d8, body+0x1990, body+0x1ae0)
        for i in range(4):
            struct.pack_into('<I', data, 0x1990+i*0x150+0x34, body+0x15a0)
            struct.pack_into('<I', data, 0x1990+i*0x150+0xc0, material)
        temporary = bytearray(0x300)
        struct.pack_into('<I', temporary, 0, solver)
        # Invalid stale counts are deliberately retained as bytes, never followed.
        return {sp: stack, body: data, view: temporary, solver: bytearray(b'\xff'*0xdd0),
                material: bytearray(0x140), 0xe85fcc: bytearray(struct.pack('<I', 1)),
                0xe85f28: bytearray(struct.pack('<I', body)), 0xd16348: bytearray(4),
                0xe85df0: bytearray(32)}

    def decode(regions):
        def read(address, count):
            for start, data in regions.items():
                if start <= address and address+count <= start+len(data):
                    return bytes(data[address-start:address-start+count])
            raise ValueError('Fixture out-of-bounds read')
        return capture.wheel_force_arguments(read, sp, 0x9df8b0, 0, 0)

    baseline = decode(fixture())
    assert baseline['solver_before_hex'] == 'ff'*0xdd0
    assert baseline['body'] == hex(body) and len(baseline['materials']) == 4
    assert all(m['address'] == hex(material) for m in baseline['materials'])
    rejected = []
    for name, region, offset, value in [
        ('caller', sp, 0, 0xa04183), ('nan_delta', sp, 8, 0x7fc00000),
        ('zero_delta', sp, 8, 0), ('unknown_owner', 0xe85f28, 0, body+4),
        ('alternative_axes', body, 0xadc, 1), ('wheel_count', body, 0x1620, 6),
        ('mode', body, 0x1598, 3), ('driven_count', body, 0x18f0, 5),
        ('foreign_driven_wheel', body, 0x18d8, body+0x1994),
        ('duplicate_driven_wheel', body, 0x18dc, body+0x1990),
        ('foreign_matrix_owner', body, 0x19c4, body+0x15a4),
        ('unreadable_view', body, 0, 0x7f0000),
        ('unreadable_solver', view, 0, 0x7f0000),
    ]:
        regions = fixture()
        struct.pack_into('<I', regions[region], offset, value)
        try:
            decode(regions)
        except ValueError:
            rejected.append(name)
        else:
            raise AssertionError(f'Accepted invalid capture: {name}')
    print(json.dumps({'synthetic_only': True, 'stale_solver_bytes_retained': True,
                      'ownership_and_boundary_rejections': rejected,
                      'reference_execution_or_capture': False}))


if __name__ == '__main__':
    main()
