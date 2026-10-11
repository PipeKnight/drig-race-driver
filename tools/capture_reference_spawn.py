"""Guarded entry/return capture using the existing x32dbg headless CLI.

Uses ordinary debugger breakpoints on the owner's private reference. No game
input or arbitrary process writes are exposed. All evidence is create-new/local.
"""
import argparse
import ctypes
from ctypes import wintypes as W
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import queue
import re
import struct
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = 'b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'
ANALYSIS = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
GAME = Path(r'C:\Users\debros\Tools\grid-research\grid-compat\grid.exe')
ENTRY = 0x9DE4E0


def exit_process_slot(raw):
    """Locate the named KERNEL32 import in the hash-verified PE32 file."""
    pe, = struct.unpack_from('<I', raw, 0x3c)
    count, = struct.unpack_from('<H', raw, pe + 6)
    opt_size, = struct.unpack_from('<H', raw, pe + 20)
    opt = pe + 24
    if raw[pe:pe + 4] != b'PE\0\0' or struct.unpack_from('<H', raw, opt)[0] != 0x10b:
        raise ValueError('Expected PE32 import table')
    base, = struct.unpack_from('<I', raw, opt + 28)
    sections = [struct.unpack_from('<IIII', raw, opt + opt_size + i * 40 + 8)
                for i in range(count)]

    def offset(rva):
        for _, start, length, position in sections:
            if start <= rva < start + length:
                return position + rva - start
        raise ValueError('Import RVA outside file-backed section')

    def string(rva):
        start = offset(rva)
        end = raw.find(b'\0', start, start + 256)
        if end < 0:
            raise ValueError('Unterminated import name')
        return raw[start:end]

    imports, length = struct.unpack_from('<II', raw, opt + 104)
    for i in range(min(length // 20, 4096)):
        original, _, _, name, first = struct.unpack_from('<5I', raw, offset(imports) + i * 20)
        if not name:
            break
        if string(name).lower() != b'kernel32.dll':
            continue
        for j in range(4096):
            thunk, = struct.unpack_from('<I', raw, offset(original or first) + j * 4)
            if not thunk:
                break
            if not thunk & 0x80000000 and string(thunk + 2) == b'ExitProcess':
                return base + first + j * 4
    raise ValueError('Named ExitProcess import absent')


def impact_arguments(read, sp, ip, ax, index, callback=False):
    """Capture the verified cdecl producer boundary; no inferred receiver ABI."""
    stack = read(sp, 32 if callback else 36)
    if callback:
        ret,linked,contacts,count,scalar,unused,response,descriptor=struct.unpack('<8I',stack)
        prepared_a=prepared_b=0
        if ret not in (0xb77bd8,0xb784fa) or unused!=0:raise ValueError('Unverified callback caller/unused argument')
    else:
        ret, linked, contacts, count, scalar, response, prepared_a, prepared_b, descriptor = struct.unpack('<9I', stack)
    if (not callback and ret != 0xa0652b) or not 1 <= count <= 80:
        raise ValueError('Impact caller/contact count outside verified scope')
    body = descriptor - 0x2300
    list_count, = struct.unpack('<I', read(0xe85fcc, 4))
    if not 1 <= list_count <= 40:
        raise ValueError('Standard body list outside scope')
    bodies = struct.unpack(f'<{list_count}I', read(0xe85f28, list_count * 4))
    if body not in bodies:
        raise ValueError('Impact descriptor does not belong to registered body')
    wrapper, = struct.unpack('<I', read(body + 0xdb0, 4))
    if struct.unpack('<I', read(wrapper + 8, 4))[0] != linked:
        raise ValueError('Impact linked state/body mismatch')
    if struct.unpack('<3I', read(linked + 0x418, 12)) != (0x9a0c90, 0xa064a0, descriptor):
        raise ValueError('Impact callback registration mismatch')
    settings, = struct.unpack('<I', read(descriptor + 0xa4f8, 4))
    pointers = struct.unpack(f'<{count}I', read(contacts, count * 4))
    contact_rows = []
    for pointer in pointers:
        value = read(pointer, 0x80)
        counterpart = struct.unpack_from('<I', value, 8)[0] if not value[0xc] else 0
        if counterpart == linked:
            counterpart = struct.unpack_from('<I', value, 4)[0]
        contact_rows.append(dict(address=hex(pointer), before_hex=value.hex(),
                                 counterpart=hex(counterpart),
                                 counterpart_360_hex=read(counterpart + 0x360, 16).hex() if counterpart else None))
    result=dict(index=index, entry_registers=[ip, sp, ax], stack_hex=stack.hex(),
                return_address=hex(ret), body=hex(body), linked=hex(linked),
                descriptor=hex(descriptor), contact_array=hex(contacts), contact_count=count,
                scalar_word=f'{scalar:08x}', response=hex(response), prepared_a=hex(prepared_a),
                prepared_b=hex(prepared_b), settings=hex(settings), contacts=contact_rows,
                descriptor_before_hex=read(descriptor, 0xa500).hex(),
                settings_hex=read(settings, 0x1050).hex(), linked_before_hex=read(linked, 0x424).hex(),
                body_1630_hex=read(body + 0x1630, 16).hex(),
                body_b20_hex=read(body + 0xb20, 16).hex(),
                prepared_a_hex=read(prepared_a, 16).hex() if prepared_a else None, prepared_b_hex=read(prepared_b, 16).hex() if prepared_b else None,
                response_before_hex=read(response, count * 4).hex() if response else None,
                solver_globals_hex=read(0xe4fc80, 24).hex(), solver_clip_hex=read(0xe500b8, 1).hex(),
                producer_corner_scale_hex=read(0xe4fc2c, 4).hex(),
                response_scale_hex=read(0xe4fc38, 4).hex(),
                producer_static_mass_hex=read(0xe51d40, 4).hex(),
                producer_globals_hex=read(0xe500c0, 32).hex())
    if callback:
        center,=struct.unpack('<I',read(linked+0x334,4))
        result.update(callback_global_disable=read(0xe85fad,1)[0],body_d16c_hex=read(body+0xd16c,4).hex(),body_d22c_hex=read(body+0xd22c,4).hex(),global_d1676c_hex=read(0xd1676c,4).hex(),
            preparation_scale_hex=read(0xe4fc30,4).hex(),preparation_table_hex=read(0xe4fc48,64).hex(),callback_body_state=dict(center_hex=read(center,16).hex(),body_290_before_hex=read(body+0x290,40).hex(),body_2e0_before_hex=read(body+0x2e0,4).hex(),body_15a8_hex=read(body+0x15a8,4).hex(),globals_hex=read(0xe500a4,20).hex()))
    return result


def private(path):
    result = path.resolve()
    if not result.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence/session must remain under local/')
    return result


def collection_arguments(read,sp,ip,ax,cx,index):
    """Verified B775D0 thiscall, ordered bounded pools and shape-disable bytes."""
    stack=read(sp,20);ret,linked,output,count_address,capacity=struct.unpack('<5I',stack)
    if ret not in (0xb787d6,0xb788f3) or capacity!=80 or struct.unpack('<I',read(count_address,4))[0]!=0:
        raise ValueError('Unverified collector caller/capacity/initial count')
    registration=struct.unpack('<3I',read(linked+0x418,12));descriptor=registration[2];body=descriptor-0x2300
    if registration[:2]!=(0x9a0c90,0xa064a0):raise ValueError('Collector owner callback mismatch')
    size,=struct.unpack('<I',read(0xe85fcc,4))
    if not 1<=size<=40 or body not in struct.unpack(f'<{size}I',read(0xe85f28,size*4)):raise ValueError('Collector owner absent from Standard bodies')
    wrapper,=struct.unpack('<I',read(body+0xdb0,4))
    if struct.unpack('<I',read(wrapper+8,4))[0]!=linked:raise ValueError('Collector owner body link mismatch')
    header=read(cx,0x13c);pools=[]
    for pointer_offset,count_offset in ((0x98,0xac),(0xa0,0xb4)):
        pointer=struct.unpack_from('<I',header,pointer_offset)[0];count=struct.unpack_from('<i',header,count_offset)[0]
        if not 0<=count<=512:raise ValueError('Collector pool outside bounded scope')
        pointers=struct.unpack(f'<{count}I',read(pointer,count*4)) if count else ()
        entries=[]
        for address in pointers:
            contact=read(address,0x80);shape_a,shape_b=struct.unpack_from('<2I',contact,0x28)
            entries.append(dict(address=hex(address),before_hex=contact.hex(),shape_a=hex(shape_a),shape_b=hex(shape_b),
                disabled_a=read(shape_a+0x230,1)[0],disabled_b=read(shape_b+0x230,1)[0]))
        pools.append(dict(pointer=hex(pointer),count=count,entries=entries))
    return dict(index=index,entry_registers=[ip,sp,ax],receiver=hex(cx),stack_hex=stack.hex(),return_address=hex(ret),
        linked=hex(linked),body=hex(body),descriptor=hex(descriptor),output_array=hex(output),count_address=hex(count_address),
        capacity=capacity,initial_count=0,manager_header_hex=header.hex(),pools=pools)


def owner_arguments(read,sp,ip,ax,cx,index):
    """Whole B77770 entry: valid owner registrations and two bounded pools."""
    stack=read(sp,12);ret,output,capacity=struct.unpack('<3I',stack)
    if ret not in (0xb7855e,0xb7872a) or capacity!=32:
        raise ValueError(f'Unverified owner collector caller/capacity: return={ret:08x} capacity={capacity}')
    header=read(cx,0x13c);pools=[];owners={}
    for pool_index,(pointer_offset,count_offset) in enumerate(((0x98,0xac),(0xa0,0xb4))):
        pointer=struct.unpack_from('<I',header,pointer_offset)[0];count=struct.unpack_from('<i',header,count_offset)[0]
        if not 0<=count<=512:raise ValueError('Owner pool outside bounded scope')
        pointers=struct.unpack(f'<{count}I',read(pointer,count*4)) if count else ()
        entries=[]
        for address in pointers:
            contact=read(address,0x80);first,second=struct.unpack_from('<2I',contact,4)
            if not first or (pool_index==1 and (not second or contact[0xc]!=0)):
                raise ValueError('Owner collector native pointer precondition failed')
            for owner in ([first] if pool_index==0 else [first,second]):
                if owner not in owners:
                    owners[owner]=dict(address=hex(owner),registration_hex=read(owner+0x418,12).hex())
                    if len(owners)>32:raise ValueError('Live manager owner output would exceed caller storage')
            entries.append(dict(address=hex(address),before_hex=contact.hex()))
        pools.append(dict(pointer=hex(pointer),count=count,entries=entries))
    return dict(index=index,entry_registers=[ip,sp,ax],receiver=hex(cx),stack_hex=stack.hex(),
        return_address=hex(ret),output_array=hex(output),capacity=capacity,manager_header_hex=header.hex(),
        alternating_order_before=read(0xf5e1a4,1)[0],owners=list(owners.values()),pools=pools)


def preparation_arguments(read,sp,ip,ax,index):
    """Verified A06050 descriptor policy between reduction and collision producer."""
    stack=read(sp,24);ret,descriptor,linked,direction,inverse,point=struct.unpack('<6I',stack)
    if ret!=0xa06505:raise ValueError('Unverified preparation caller')
    body=descriptor-0x2300;size,=struct.unpack('<I',read(0xe85fcc,4))
    if not 1<=size<=40 or body not in struct.unpack(f'<{size}I',read(0xe85f28,size*4)):raise ValueError('Preparation owner absent from Standard bodies')
    wrapper,=struct.unpack('<I',read(body+0xdb0,4))
    if struct.unpack('<I',read(wrapper+8,4))[0]!=linked or struct.unpack('<3I',read(linked+0x418,12))!=(0x9a0c90,0xa064a0,descriptor):raise ValueError('Preparation body/callback registration mismatch')
    settings,=struct.unpack('<I',read(descriptor+0xa4f8,4))
    return dict(index=index,entry_registers=[ip,sp,ax],stack_hex=stack.hex(),return_address=hex(ret),body=hex(body),linked=hex(linked),descriptor=hex(descriptor),
        direction_address=hex(direction),point_address=hex(point),inverse_address=hex(inverse),inverse_hex=read(inverse,64).hex(),
        direction_before_hex=read(direction,16).hex(),point_before_hex=read(point,16).hex(),linked_before_hex=read(linked,0x424).hex(),
        descriptor_before_hex=read(descriptor,0xa500).hex(),settings_hex=read(settings,0x1050).hex(),
        body_d16c_hex=read(body+0xd16c,4).hex(),body_d22c_hex=read(body+0xd22c,4).hex(),global_d1676c_hex=read(0xd1676c,4).hex(),
        preparation_scale_hex=read(0xe4fc30,4).hex(),preparation_table_hex=read(0xe4fc48,64).hex())


def wheel_force_arguments(read, sp, ip, ax, index):
    """Read the selected whole cdecl force update while its stack view is live."""
    stack = read(sp, 16)
    ret, body, delta_word = struct.unpack_from('<3I', stack)
    delta = struct.unpack('<f', struct.pack('<I', delta_word))[0]
    if ret != 0xa04182 or not math.isfinite(delta) or not 0 < delta <= .25:
        raise ValueError('Unverified wheel-force caller/delta')
    count, = struct.unpack('<I', read(0xe85fcc, 4))
    if not 1 <= count <= 40 or body not in struct.unpack(f'<{count}I', read(0xe85f28, count*4)):
        raise ValueError('Wheel-force owner absent from Standard bodies')
    before = read(body, 0x2600)
    if before[0xadc] != 0 or struct.unpack_from('<I', before, 0x1620)[0] != 4:
        raise ValueError('Wheel-force selection/wheel count outside scope')
    mode, = struct.unpack_from('<I', before, 0x1598)
    driven_count, = struct.unpack_from('<I', before, 0x18f0)
    driven = struct.unpack_from(f'<{driven_count}I', before, 0x18d8) if driven_count <= 4 else ()
    wheel_addresses = {body+0x1990+i*0x150 for i in range(4)}
    if mode > 2 or driven_count > 4 or len(set(driven)) != driven_count or any(p not in wheel_addresses for p in driven):
        raise ValueError('Wheel-force mode/driven ownership outside scope')
    materials = []
    for i in range(4):
        wheel = 0x1990+i*0x150
        if struct.unpack_from('<I', before, wheel+0x34)[0] != body+0x15a0:
            raise ValueError('Wheel-force matrix owner mismatch')
        # 9B5660 follows wheel+C0 and reads pointed+64, not wheel+64.
        pointer, = struct.unpack_from('<I', before, wheel+0xc0)
        materials.append(dict(wheel=i, address=hex(pointer), before_hex=read(pointer, 0x140).hex() if pointer else None))
    view, = struct.unpack_from('<I', before)
    view_before = read(view, 0x300)
    solver, = struct.unpack_from('<I', view_before)
    # Entry storage may contain stale counts from another car. Preserve raw bytes;
    # never interpret them or read variable-length buffers before this call resets it.
    solver_before = read(solver, 0xdd0)
    return dict(index=index, entry_registers=[ip, sp, ax], stack_hex=stack.hex(),
                return_address=hex(ret), body=hex(body), delta_word=f'{delta_word:08x}',
                view=hex(view), solver=hex(solver), mode=mode, driven_count=driven_count,
                body_before_hex=before.hex(), view_before_hex=view_before.hex(),
                solver_before_hex=solver_before.hex(), materials=materials,
                engine_limit_hex=read(0xd16348, 4).hex(), friction_diagnostics_before_hex=read(0xe85df0, 32).hex())


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('session', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cases', type=int, default=12)
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--arm-running', action='store_true',
                        help='Arm after attach without the old debugger pause command')
    parser.add_argument('--boundary', choices=('placement', 'impact','collection','owners','preparation','callback','wheel-force'), default='placement',
                        help='Select a guarded entry/return boundary; wheel-force captures whole 9DF8B0')
    parser.add_argument('--force-mode', type=int, choices=(0, 1, 2),
                        help='Restrict wheel-force entry stops to the selected body mode')
    parser.add_argument('--hardware', action='store_true',
                        help='Use bounded execute debug registers, leaving code bytes unchanged')
    parser.add_argument('--solver-records', action='store_true',
                        help='Also capture the nested positive-record 9D4970 solver entry/return')
    parser.add_argument('--body-effects', action='store_true',
                        help='Capture9D4210 inputs/return and its optional original random result before solver')
    args = parser.parse_args()
    entry = {'placement':ENTRY,'impact':0x9ea6e0,'collection':0xb775d0,'owners':0xb77770,'preparation':0xa06050,'callback':0xa064a0,'wheel-force':0x9df8b0}[args.boundary]
    if args.boundary != 'placement' and not args.arm_running:
        raise ValueError('Impact capture requires the previously validated arm-running workflow')
    if args.solver_records and (args.boundary not in ('impact','callback') or not args.hardware):
        raise ValueError('Nested solver records require hardware impact capture')
    if args.body_effects and not args.solver_records:
        raise ValueError('Body effects require hardware impact with nested solver capture')
    if args.boundary=='collection' and not args.hardware:raise ValueError('Collector capture requires hardware breakpoints')
    if args.boundary=='owners' and not args.hardware:raise ValueError('Owner capture requires hardware breakpoints')
    if args.boundary=='preparation' and not args.hardware:raise ValueError('Preparation capture requires hardware breakpoints')
    if args.boundary=='callback' and not args.hardware:raise ValueError('Callback capture requires hardware breakpoints')
    if args.boundary=='wheel-force' and not args.hardware:raise ValueError('Wheel-force capture requires hardware breakpoints')
    if args.force_mode is not None and args.boundary!='wheel-force':
        raise ValueError('Force mode requires wheel-force boundary')
    force_condition='[csp]==A04182'
    if args.force_mode is not None:force_condition+=f'&&[[csp+4]+1598]=={args.force_mode:X}'
    if not 1 <= args.cases <= 32 or not 30 <= args.timeout <= 900:
        raise ValueError('Case/time limit outside scope')
    session = json.loads(private(args.session).read_text(encoding='utf-8-sig'))
    if Path(session['Path']) != GAME or session['ExeSHA256'] != REFERENCE:
        raise ValueError('Unexpected reference session')
    if hashlib.sha256(GAME.read_bytes()).hexdigest() != REFERENCE:
        raise ValueError('Reference file changed')
    raw = (ROOT / 'local/compatibility/grid-unpacked.exe').read_bytes()
    if hashlib.sha256(raw).hexdigest() != ANALYSIS:
        raise ValueError('Analysis file changed')
    out = private(args.output)
    out.mkdir(parents=True, exist_ok=False)
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
    k.OpenProcess.restype = W.HANDLE
    k.CloseHandle.argtypes = [W.HANDLE]
    k.GetProcessTimes.argtypes = [W.HANDLE] + [ctypes.POINTER(W.FILETIME)] * 4
    k.QueryFullProcessImageNameW.argtypes = [W.HANDLE, W.DWORD, W.LPWSTR, ctypes.POINTER(W.DWORD)]
    k.ReadProcessMemory.argtypes = [W.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                  ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
    handle = k.OpenProcess(0x410, False, int(session['Pid']))
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    process = None
    stream = None
    record = {'session': session, 'entry': hex(entry), 'boundary': args.boundary, 'cases': [],
              'input_sent': False, 'debugger_pauses': False, 'completed': False}
    record['arm_running'] = args.arm_running
    record['hardware_breakpoints'] = args.hardware
    record['solver_records'] = args.solver_records
    record['body_effects'] = args.body_effects
    record['force_mode'] = args.force_mode
    try:
        path_buffer = ctypes.create_unicode_buffer(32768)
        path_length = W.DWORD(len(path_buffer))
        if not k.QueryFullProcessImageNameW(handle, 0, path_buffer, ctypes.byref(path_length)):
            raise ctypes.WinError(ctypes.get_last_error())
        if Path(path_buffer.value) != GAME:
            raise ValueError('Live process path changed')
        times = [W.FILETIME() for _ in range(4)]
        if not k.GetProcessTimes(handle, *(ctypes.byref(t) for t in times)):
            raise ctypes.WinError(ctypes.get_last_error())
        actual_ticks = times[0].dwLowDateTime | (times[0].dwHighDateTime << 32)
        seconds, fraction = session['StartedUtc'].rstrip('Z').split('.')
        delta = datetime.fromisoformat(seconds).replace(tzinfo=timezone.utc) - datetime(1601, 1, 1, tzinfo=timezone.utc)
        expected_ticks = (delta.days * 86400 + delta.seconds) * 10000000 + int(fraction.ljust(7, '0'))
        if actual_ticks != expected_ticks:
            raise ValueError('Live process start ticks changed')

        def read(address, length):
            if not 0x10000 <= address < 0x7FFF0000 or not 0 < length <= 65536:
                raise ValueError('Memory read outside scope')
            buffer = ctypes.create_string_buffer(length)
            count = ctypes.c_size_t()
            if not k.ReadProcessMemory(handle, address, buffer, length, ctypes.byref(count)) or count.value != length:
                raise ctypes.WinError(ctypes.get_last_error())
            return buffer.raw

        offset = entry - 0x400000 - 0x1000 + 0x400
        if read(entry, 32) != raw[offset:offset + 32]:
            raise ValueError('Live placement code differs')
        record['code_exact'] = True
        if args.solver_records:
            solver_offset = 0x9d4970 - 0x400000 - 0x1000 + 0x400
            if read(0x9d4970, 32) != raw[solver_offset:solver_offset + 32]:
                raise ValueError('Live solver code differs')
            record['solver_code_exact'] = True
        if args.body_effects:
            for address in (0x9d4210,0x9d46fd):
                body_offset=address-0x400000-0x1000+0x400
                if read(address,32)!=raw[body_offset:body_offset+32]:
                    raise ValueError('Live body-effect code differs')
            record['body_code_exact']=True
        exit_address = None
        if args.boundary != 'placement':
            exit_slot = exit_process_slot(raw)
            exit_address, = struct.unpack('<I', read(exit_slot, 4))
            if not 0x10000000 <= exit_address < 0x7fff0000:
                raise ValueError('ExitProcess import outside expected system scope')
            record['exit_watch'] = {'iat': hex(exit_slot), 'address': hex(exit_address),
                                    'prefix_hex': read(exit_address, 32).hex()}
        debugger = ROOT / 'local/tools/x64dbg/release/x32/headless.exe'
        record['debugger_sha256'] = hashlib.sha256(debugger.read_bytes()).hexdigest()
        stream = (out / 'debugger.log').open('x', encoding='utf-8')
        commands = (out / 'commands.log').open('x', encoding='utf-8')
        messages = queue.Queue()
        debugging_stopped = threading.Event()
        hardware_stop = threading.Event()
        last_exception = {}
        (out / 'debugger-user').mkdir()
        process = subprocess.Popen([str(debugger), '-userdir', str(out / 'debugger-user')],
                                   cwd=str(debugger.parent), stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   text=True, encoding='utf-8', errors='replace', bufsize=1,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        record['debugger_pid'] = process.pid
        record['debugger_pauses'] = True

        def receive():
            for line in process.stdout:
                stream.write(line)
                stream.flush()
                if 'Debugging stopped!' in line:
                    debugging_stopped.set()
                if 'Hardware breakpoint (execute)' in line:
                    hardware_stop.set()
                if '[STATE] running' in line or 'First chance exception on ' in line:
                    hardware_stop.clear()
                match = re.search(r'First chance exception on ([0-9A-Fa-f]+) \(([0-9A-Fa-f]+),', line)
                if match:
                    last_exception.clear()
                    last_exception.update(ip=int(match[1], 16), code=int(match[2], 16))
                messages.put(line)

        thread = threading.Thread(target=receive, daemon=True)
        thread.start()

        def send(command):
            commands.write(command + '\n')
            commands.flush()
            process.stdin.write(command + '\n')
            process.stdin.flush()

        def wait(marker, timeout, cleanup=False):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if not cleanup and (out / 'stop').exists():
                    raise RuntimeError('Requested stop')
                try:
                    line = messages.get(timeout=0.2)
                    if '[FAIL]' in line:
                        raise RuntimeError('Debugger command failed: ' + line.strip())
                    if 'Process stopped with exit code' in line:
                        raise RuntimeError('Reference exited: ' + line.strip())
                    if marker in line:
                        return line.strip()
                except queue.Empty:
                    if process.poll() is not None:
                        raise RuntimeError('Debugger exited')
            raise TimeoutError('Waiting for ' + marker)

        def registers(tag):
            send(f'log "{tag} ip={{p:cip}} sp={{p:csp}} ax={{p:cax}}"')
            line = wait(tag + ' ip=', 10)
            found = re.search(r'ip=([0-9A-Fa-f]+) sp=([0-9A-Fa-f]+) ax=([0-9A-Fa-f]+)', line)
            if not found:
                raise ValueError('Invalid register marker: ' + line)
            return tuple(int(v, 16) for v in found.groups())

        def breakpoint(address, name):
            send(f'bph {address:X},x,1' if args.hardware else f'bp {address:X},{name},ss')

        def prepare_detach():
            if hardware_stop.is_set():
                # Consume only our debugger-owned hardware exception. Detaching
                # directly while erun's policy is active can deliver 80000004 to
                # the target. Resume the same instruction; never skip code.
                send('bphc')
                send('con')
                wait('Exception will be swallowed', 5, cleanup=True)
                send('run')
                wait('[STATE] running', 5, cleanup=True)
                record['hardware_stop_resumed_before_detach'] = True

        wait('[headless] entering command loop', 15)
        send(f'attach {int(session["Pid"]):X}')
        if args.arm_running:
            wait('Attached to process!', 30)
        else:
            try:
                wait('[STATE] paused', 5)
            except TimeoutError:
                send('pause')
                wait('[STATE] paused', 20)
            record['attach_registers'] = registers('GRID_SPAWN_ATTACH')
        breakpoint(entry, 'GRID_Reference')
        if args.boundary=='collection':send('bphwcond B775D0,[[csp+4]+41C]==A064A0')
        if args.boundary=='preparation':send('bphwcond A06050,[csp]==A06505')
        if args.boundary=='wheel-force':send('bphwcond 9DF8B0,'+force_condition)
        if exit_address is not None:
            breakpoint(exit_address, 'GRID_ExitProcess')
        send('log "GRID_SPAWN_ARMED"')
        wait('GRID_SPAWN_ARMED', 10)
        if not args.arm_running:
            send('erun')
            wait('[STATE] running', 10)
        with (out / 'ready.json').open('x', encoding='utf-8') as f:
            json.dump({'pid': session['Pid'], 'debugger_pid': process.pid, 'entry': hex(entry)}, f)
        print(f'READY: {args.boundary} entry armed; private output=' + str(out), flush=True)
        for index in range(args.cases):
            deadline = time.monotonic() + args.timeout
            while True:
                wait('[STATE] paused', max(0, deadline - time.monotonic()))
                ip, sp, ax = registers(f'GRID_SPAWN_ENTRY_{index}')
                if ip == entry:
                    if args.hardware:
                        send(f'bphc {entry:X}')
                    break
                if ip == exit_address:
                    stack = read(sp, 256)
                    caller, exit_code = struct.unpack_from('<2I', stack)
                    record['exit_boundary'] = {'registers': [ip, sp, ax],
                                               'caller': hex(caller), 'exit_code': exit_code,
                                               'stack_hex': stack.hex()}
                    raise RuntimeError(f'Reference requested ExitProcess({exit_code}) from {caller:08x}')
                # Only the observed C++ first-chance event may be passed.
                # Unknown DLL stops/access violations are retained and aborted.
                if not args.arm_running or index != 0 or last_exception != {'ip': ip, 'code': 0xe06d7363}:
                    raise RuntimeError(f'Unexpected debugger stop at {ip:08x}')
                record.setdefault('pre_entry_stops', []).append({'ip': hex(ip), 'sp': hex(sp)})
                if len(record['pre_entry_stops']) > 16:
                    raise RuntimeError('Pre-entry exception/attach stop bound')
                send('erun')
                wait('[STATE] running', 10)
            if args.boundary in ('impact','callback'):
                case = impact_arguments(read, sp, ip, ax, index,callback=args.boundary=='callback')
                ret = int(case['return_address'], 16)
            elif args.boundary=='collection':
                send('log "GRID_COLLECTION_RECEIVER cx={p:ecx}"')
                match=re.search(r'cx=([0-9A-Fa-f]+)',wait('GRID_COLLECTION_RECEIVER cx=',10))
                if not match:raise ValueError('Missing collector receiver')
                case=collection_arguments(read,sp,ip,ax,int(match[1],16),index);ret=int(case['return_address'],16)
            elif args.boundary=='owners':
                send('log "GRID_OWNER_RECEIVER cx={p:ecx}"')
                match=re.search(r'cx=([0-9A-Fa-f]+)',wait('GRID_OWNER_RECEIVER cx=',10))
                if not match:raise ValueError('Missing owner receiver')
                case=owner_arguments(read,sp,ip,ax,int(match[1],16),index);ret=int(case['return_address'],16)
            elif args.boundary=='preparation':
                case=preparation_arguments(read,sp,ip,ax,index);ret=int(case['return_address'],16)
            elif args.boundary=='wheel-force':
                case=wheel_force_arguments(read,sp,ip,ax,index);ret=int(case['return_address'],16)
                if args.force_mode is not None and case['mode']!=args.force_mode:
                    raise ValueError('Force mode breakpoint condition mismatch')
            else:
                stack = read(sp, 28)
                words = struct.unpack('<7I', stack)
                ret, pose, body, x, z, heading, upper = words
                wheel_count = struct.unpack('<I', read(body + 0x1620, 4))[0]
                if wheel_count != 4 or not 0x400000 <= ret < 0xC00000:
                    raise ValueError('Unverified placement arguments')
                case = {'index': index, 'entry_registers': [ip, sp, ax], 'stack_hex': stack.hex(),
                        'return': hex(ret), 'pose_address': hex(pose), 'body': hex(body),
                        'heading_hex': read(heading, 16).hex() if heading else None,
                        'wheels_hex': read(body + 0x1990, 4 * 0x150).hex(),
                        'initial_geometry_hex': read(body + 0x14E0, 0x50).hex(),
                        'bounds_hex': read(body + 0xED64, 0x40).hex()}
            record['cases'].append(case)
            breakpoint(ret, 'GRID_PlacementReturn')
            if args.boundary=='wheel-force':
                # Other vehicle workers use the same caller. Stop only when our
                # captured frame has returned, before its caller removes arguments.
                send(f'bphwcond {ret:X},csp=={sp+4:X}')
            if args.solver_records:
                breakpoint(0x9d4970, 'GRID_NestedSolver')
                # Only this producer's verified call site and descriptor may stop.
                send(f'bphwcond 9D4970,[csp]==9EB570&&ecx=={int(case["descriptor"],16):X}')
            if args.body_effects:
                breakpoint(0x9d4210,'GRID_BodyEffects')
                send(f'bphwcond 9D4210,[csp]==9EB553&&[csp+4]=={int(case["linked"],16):X}')
            send('erun')
            wait('[STATE] running', 10)
            wait('[STATE] paused', 30)
            returned = registers(f'GRID_SPAWN_RETURN_{index}')
            if args.body_effects:
                send('bphc 9D4210')
                if returned[0]==0x9d4210:
                    body_sp=returned[1]
                    body_stack=read(body_sp,20)
                    body_ret,owner,frame,prepared_a,prepared_b=struct.unpack('<5I',body_stack)
                    if body_ret!=0x9eb553 or owner!=int(case['linked'],16) or (args.boundary!='callback' and (prepared_a!=int(case['prepared_a'],16) or prepared_b!=int(case['prepared_b'],16))):
                        raise ValueError('Body-effect caller/arguments mismatch')
                    if args.boundary=='callback':
                        case.update(prepared_a=hex(prepared_a),prepared_b=hex(prepared_b),prepared_a_hex=read(prepared_a,16).hex(),prepared_b_hex=read(prepared_b,16).hex())
                    center,=struct.unpack_from('<I',read(owner+0x334,4))
                    body=int(case['body'],16)
                    case['body_effect']=dict(entry_registers=returned,stack_hex=body_stack.hex(),frame_hex=read(frame,64).hex(),
                        linked_before_hex=read(owner,0x424).hex(),center_address=hex(center),center_hex=read(center,16).hex(),
                        body_290_before_hex=read(body+0x290,40).hex(),body_2e0_before_hex=read(body+0x2e0,4).hex(),body_15a8_hex=read(body+0x15a8,4).hex(),
                        globals_hex=read(0xe500a4,20).hex(),prepared_a_before_hex=read(prepared_a,16).hex(),prepared_b_hex=read(prepared_b,16).hex(),random_result=None)
                    # Keep ExitProcess and outer return; release solver entry so
                    # body return plus optional random return fit four DR slots.
                    send('bphc 9D4970')
                    breakpoint(body_ret,'GRID_BodyReturn')
                    breakpoint(0x9d46fd,'GRID_BodyRandomReturn')
                    send('erun');wait('[STATE] running',10);wait('[STATE] paused',30)
                    body_returned=registers(f'GRID_BODY_RETURN_{index}')
                    random_hit=body_returned[0]==0x9d46fd
                    if random_hit:
                        if body_returned[1] >= body_sp or body_sp-body_returned[1]>0x200:
                            raise ValueError('Body random return stack outside frame')
                        case['body_effect']['random_result']=body_returned[2]
                        case['body_effect']['random_registers']=body_returned
                        send('bphc 9D46FD');send('erun');wait('[STATE] running',10);wait('[STATE] paused',30)
                        body_returned=registers(f'GRID_BODY_COMPLETE_{index}')
                    if body_returned[0]!=body_ret or body_returned[1]!=body_sp+4:
                        raise RuntimeError('Body return ABI mismatch')
                    if not random_hit:send('bphc 9D46FD')
                    case['body_effect'].update(return_registers=body_returned,linked_after_hex=read(owner,0x424).hex(),
                        body_290_after_hex=read(body+0x290,40).hex(),body_2e0_after_hex=read(body+0x2e0,4).hex(),prepared_a_after_hex=read(prepared_a,16).hex())
                    send(f'bphc {body_ret:X}')
                    breakpoint(0x9d4970,'GRID_NestedSolver')
                    send(f'bphwcond 9D4970,[csp]==9EB570&&ecx=={int(case["descriptor"],16):X}')
                    send('erun');wait('[STATE] running',10);wait('[STATE] paused',30)
                    returned=registers(f'GRID_AFTER_BODY_{index}')
                elif returned[0]==ret:
                    case['body_effect']=None
                else:
                    raise RuntimeError('Unexpected body-effect boundary')
            if args.solver_records:
                if returned[0] == 0x9d4970:
                    solver_sp = returned[1]
                    solver_stack = read(solver_sp, 16)
                    solver_ret, records, count, gain = struct.unpack('<4I', solver_stack)
                    send('log "GRID_SOLVER_RECEIVER cx={p:ecx}"')
                    receiver_line = wait('GRID_SOLVER_RECEIVER cx=', 10)
                    receiver_match = re.search(r'cx=([0-9A-Fa-f]+)', receiver_line)
                    if not receiver_match:
                        raise ValueError('Missing solver receiver')
                    receiver = int(receiver_match[1], 16)
                    if solver_ret != 0x9eb570 or receiver != int(case['descriptor'],16) or not 1 <= count <= 400:
                        raise ValueError('Nested solver caller/receiver/count mismatch')
                    case['solver'] = dict(entry_registers=returned, receiver=hex(receiver),
                        stack_hex=solver_stack.hex(), record_count=count, gain_word=f'{gain:08x}',
                        records_hex=read(records, count*48).hex(),
                        descriptor_before_hex=read(receiver, 0xa500).hex(),
                        settings_hex=read(int(case['settings'],16), 0x1050).hex(),
                        globals_hex=read(0xe4fc80, 24).hex(), clip_hex=read(0xe500b8, 1).hex())
                    send('bphc 9D4970')
                    breakpoint(solver_ret, 'GRID_SolverReturn')
                    send('erun')
                    wait('[STATE] running', 10)
                    wait('[STATE] paused', 30)
                    solver_returned = registers(f'GRID_SOLVER_RETURN_{index}')
                    if solver_returned[0] != solver_ret or solver_returned[1] != solver_sp + 16:
                        raise RuntimeError('Nested solver return ABI mismatch')
                    case['solver']['return_registers'] = solver_returned
                    case['solver']['descriptor_after_hex'] = read(receiver, 0xa500).hex()
                    send(f'bphc {solver_ret:X}')
                    send('erun')
                    wait('[STATE] running', 10)
                    wait('[STATE] paused', 30)
                    returned = registers(f'GRID_OUTER_RETURN_{index}')
                else:
                    case['solver'] = None
                    send('bphc 9D4970')
            if returned[0] != ret or returned[1] != sp + ({'collection':20,'owners':12}.get(args.boundary,4)):
                raise RuntimeError('Return breakpoint stack/address mismatch')
            if args.hardware:
                send(f'bphc {ret:X}')
            case['return_registers'] = returned
            if args.boundary in ('impact','callback'):
                case['descriptor_after_hex'] = read(int(case['descriptor'], 16), 0xa500).hex()
                case['linked_after_hex'] = read(int(case['linked'], 16), 0x424).hex()
                if int(case['response'], 16):
                    case['response_after_hex'] = read(int(case['response'], 16), case['contact_count'] * 4).hex()
                for contact in case['contacts']:
                    contact['after_hex'] = read(int(contact['address'], 16), 0x80).hex()
                if args.boundary=='callback':
                    case['callback_result']=returned[2]&255
                    case['callback_body_state'].update(body_290_after_hex=read(int(case['body'],16)+0x290,40).hex(),body_2e0_after_hex=read(int(case['body'],16)+0x2e0,4).hex())
            elif args.boundary=='collection':
                count,=struct.unpack('<I',read(int(case['count_address'],16),4))
                if not 0<=count<=case['capacity']:raise ValueError('Collector output count outside scope')
                pointers=struct.unpack(f'<{count}I',read(int(case['output_array'],16),count*4)) if count else ()
                known={int(e['address'],16) for p in case['pools'] for e in p['entries']}
                if any(p not in known for p in pointers):raise ValueError('Collector returned unknown contact')
                case['contact_count']=count;case['output_pointers']=[hex(p) for p in pointers]
            elif args.boundary=='owners':
                count=returned[2]
                if not 0<=count<=32:raise ValueError('Owner count exceeds live caller storage')
                pointers=struct.unpack(f'<{count}I',read(int(case['output_array'],16),count*4)) if count else ()
                known={int(o['address'],16) for o in case['owners']}
                if any(p not in known for p in pointers) or len(set(pointers))!=count:
                    raise ValueError('Unknown or duplicate returned owner')
                case['owner_count']=count;case['output_pointers']=[hex(p) for p in pointers]
            elif args.boundary=='preparation':
                case['descriptor_after_hex']=read(int(case['descriptor'],16),0xa500).hex()
                case['linked_after_hex']=read(int(case['linked'],16),0x424).hex()
                case['direction_after_hex']=read(int(case['direction_address'],16),16).hex()
                case['point_after_hex']=read(int(case['point_address'],16),16).hex()
                case['inverse_after_hex']=read(int(case['inverse_address'],16),64).hex()
            elif args.boundary=='wheel-force':
                body, view, solver = (int(case[k], 16) for k in ('body', 'view', 'solver'))
                after = read(body, 0x2600)
                if struct.unpack_from('<I', after)[0] != view or struct.unpack('<I', read(view, 4))[0] != solver:
                    raise ValueError('Wheel-force temporary storage identity changed before return')
                case['body_after_hex'] = after.hex()
                case['view_after_hex'] = read(view, 0x300).hex()
                case['solver_after_hex'] = read(solver, 0xdd0).hex()
                case['friction_diagnostics_after_hex'] = read(0xe85df0, 32).hex()
                for material in case['materials']:
                    pointer = int(material['address'], 16)
                    material['after_hex'] = read(pointer, 0x140).hex() if pointer else None
            else:
                case['pose_hex'] = read(pose, 64).hex()
            with (out / f'case-{index:02}.json').open('x', encoding='utf-8') as f:
                json.dump(case, f, indent=2)
            print(f'Captured {args.boundary} {index + 1}/{args.cases}', flush=True)
            if index + 1 < args.cases:
                breakpoint(entry, 'GRID_Reference')
                if args.boundary=='collection':send('bphwcond B775D0,[[csp+4]+41C]==A064A0')
                if args.boundary=='preparation':send('bphwcond A06050,[csp]==A06505')
                if args.boundary=='wheel-force':send('bphwcond 9DF8B0,'+force_condition)
                send('erun')
                wait('[STATE] running', 10)
        record['completed'] = True
        prepare_detach()
        send('detach')
        wait('Debugging stopped!', 20)
        send('exit')
        process.wait(timeout=15)
        thread.join(timeout=3)
        commands.close()
    except Exception as error:
        record['error'] = repr(error)
        print('CAPTURE_FAILED: ' + repr(error), flush=True)
        raise
    finally:
        if process is not None and process.poll() is None:
            try:
                if not debugging_stopped.is_set():
                    prepare_detach()
                    send('detach')
                deadline = time.monotonic() + 20
                detached = debugging_stopped.is_set()
                while time.monotonic() < deadline:
                    if detached:
                        break
                    try:
                        line = messages.get(timeout=0.2)
                        if 'Debugging stopped!' in line:
                            detached = True
                            break
                    except queue.Empty:
                        if process.poll() is not None:
                            break
                if not detached and process.poll() is None:
                    raise TimeoutError('Detach completion not verified')
                process.stdin.write('exit\n')
                process.stdin.flush()
                process.wait(timeout=15)
            except (OSError, subprocess.TimeoutExpired, TimeoutError, RuntimeError):
                record['cleanup_uncertain'] = True
                # Do not terminate an attached debugger: target survival takes priority.
                print('Debugger cleanup uncertain; inspect recorded identity', flush=True)
        k.CloseHandle(handle)
        if stream is not None and (process is None or process.poll() is not None):
            stream.close()
        record['debugger_exit'] = None if process is None else process.poll()
        with (out / 'capture.json').open('x', encoding='utf-8') as f:
            json.dump(record, f, indent=2)


if __name__ == '__main__':
    main()
