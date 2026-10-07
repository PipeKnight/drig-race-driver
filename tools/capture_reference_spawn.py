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


def private(path):
    result = path.resolve()
    if not result.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence/session must remain under local/')
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('session', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cases', type=int, default=12)
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--arm-running', action='store_true',
                        help='Arm after attach without the old debugger pause command')
    args = parser.parse_args()
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
    record = {'session': session, 'entry': hex(ENTRY), 'cases': [],
              'input_sent': False, 'debugger_pauses': True, 'completed': False}
    record['arm_running'] = args.arm_running
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

        offset = ENTRY - 0x400000 - 0x1000 + 0x400
        if read(ENTRY, 32) != raw[offset:offset + 32]:
            raise ValueError('Live placement code differs')
        record['code_exact'] = True
        debugger = ROOT / 'local/tools/x64dbg/release/x32/headless.exe'
        record['debugger_sha256'] = hashlib.sha256(debugger.read_bytes()).hexdigest()
        stream = (out / 'debugger.log').open('x', encoding='utf-8')
        commands = (out / 'commands.log').open('x', encoding='utf-8')
        messages = queue.Queue()
        debugging_stopped = threading.Event()
        last_exception = {}
        (out / 'debugger-user').mkdir()
        process = subprocess.Popen([str(debugger), '-userdir', str(out / 'debugger-user')],
                                   cwd=str(debugger.parent), stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   text=True, encoding='utf-8', errors='replace', bufsize=1,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        record['debugger_pid'] = process.pid

        def receive():
            for line in process.stdout:
                stream.write(line)
                stream.flush()
                if 'Debugging stopped!' in line:
                    debugging_stopped.set()
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

        def wait(marker, timeout):
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if (out / 'stop').exists():
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
        send(f'bp {ENTRY:X},GRID_Placement,ss')
        send('log "GRID_SPAWN_ARMED"')
        wait('GRID_SPAWN_ARMED', 10)
        if not args.arm_running:
            send('erun')
            wait('[STATE] running', 10)
        with (out / 'ready.json').open('x', encoding='utf-8') as f:
            json.dump({'pid': session['Pid'], 'debugger_pid': process.pid, 'entry': hex(ENTRY)}, f)
        print('READY: placement entry armed; private output=' + str(out), flush=True)
        for index in range(args.cases):
            deadline = time.monotonic() + args.timeout
            while True:
                wait('[STATE] paused', max(0, deadline - time.monotonic()))
                ip, sp, ax = registers(f'GRID_SPAWN_ENTRY_{index}')
                if ip == ENTRY:
                    break
                # Only the observed C++ first-chance event may be passed.
                # Unknown DLL stops/access violations are retained and aborted.
                if not args.arm_running or index != 0 or last_exception != {'ip': ip, 'code': 0xe06d7363}:
                    raise RuntimeError(f'Unexpected debugger stop at {ip:08x}')
                record.setdefault('pre_entry_stops', []).append({'ip': hex(ip), 'sp': hex(sp)})
                if len(record['pre_entry_stops']) > 16:
                    raise RuntimeError('Pre-entry exception/attach stop bound')
                send('erun')
                wait('[STATE] running', 10)
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
            send(f'bp {ret:X},GRID_PlacementReturn,ss')
            send('erun')
            wait('[STATE] running', 10)
            wait('[STATE] paused', 30)
            returned = registers(f'GRID_SPAWN_RETURN_{index}')
            if returned[0] != ret or returned[1] != sp + 4:
                raise RuntimeError('Return breakpoint stack/address mismatch')
            case['return_registers'] = returned
            case['pose_hex'] = read(pose, 64).hex()
            with (out / f'case-{index:02}.json').open('x', encoding='utf-8') as f:
                json.dump(case, f, indent=2)
            print(f'Captured placement {index + 1}/{args.cases}, body={body:08x}', flush=True)
            if index + 1 < args.cases:
                send(f'bp {ENTRY:X},GRID_Placement,ss')
                send('erun')
                wait('[STATE] running', 10)
        record['completed'] = True
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
                    process.stdin.write('detach\n')
                    process.stdin.flush()
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
            except (OSError, subprocess.TimeoutExpired):
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
