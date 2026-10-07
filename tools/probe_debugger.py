"""Attach x32dbg headless to one owned GRID startup and inspect a known call site."""
import ctypes
from ctypes import wintypes as W
import json
from pathlib import Path
import queue
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'local' / 'phase1'
DEBUGGER = ROOT / 'local' / 'tools' / 'x64dbg' / 'release' / 'x32' / 'headless.exe'


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [('dwSize', W.DWORD), ('cntUsage', W.DWORD), ('th32ProcessID', W.DWORD),
                ('th32DefaultHeapID', ctypes.c_size_t), ('th32ModuleID', W.DWORD),
                ('cntThreads', W.DWORD), ('th32ParentProcessID', W.DWORD),
                ('pcPriClassBase', W.LONG), ('dwFlags', W.DWORD), ('szExeFile', W.WCHAR * 260)]


def grid_pids():
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.CreateToolhelp32Snapshot.argtypes = [W.DWORD, W.DWORD]
    k.CreateToolhelp32Snapshot.restype = W.HANDLE
    k.Process32FirstW.argtypes = [W.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
    k.Process32NextW.argtypes = [W.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
    k.CloseHandle.argtypes = [W.HANDLE]
    snapshot = k.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    entry = PROCESSENTRY32W()
    entry.dwSize = ctypes.sizeof(entry)
    result = []
    try:
        ok = k.Process32FirstW(snapshot, ctypes.byref(entry))
        while ok:
            if entry.szExeFile.lower() == 'grid.exe':
                result.append(entry.th32ProcessID)
            ok = k.Process32NextW(snapshot, ctypes.byref(entry))
    finally:
        k.CloseHandle(snapshot)
    return result


def main():
    if grid_pids():
        raise RuntimeError('GRID is already running; do not attach to an existing user session')
    process = subprocess.Popen([str(DEBUGGER)], cwd=str(DEBUGGER.parent),
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, encoding='utf-8', errors='replace', bufsize=1,
                               creationflags=subprocess.CREATE_NO_WINDOW)
    messages = queue.Queue()
    lines = []
    def reader():
        for line in process.stdout:
            lines.append(line)
            messages.put(line)
    thread = threading.Thread(target=reader, daemon=True)
    thread.start()
    def send(command):
        process.stdin.write(command + '\n')
        process.stdin.flush()
    def wait_for(marker, seconds):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            try:
                line = messages.get(timeout=min(0.2, max(0.01, until - time.monotonic())))
                if marker in line:
                    return line.strip()
            except queue.Empty:
                if process.poll() is not None:
                    break
        return None
    result = dict(target_pid=None, attached=False, breakpoint_hit=False)
    try:
        if not wait_for('[headless] entering command loop', 15):
            raise RuntimeError('Debugger did not become ready')
        subprocess.Popen([r'C:\Program Files (x86)\Steam\steam.exe', '-applaunch', '12750'], creationflags=subprocess.CREATE_NO_WINDOW)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            pids = grid_pids()
            if pids:
                break
            time.sleep(0.02)
        else:
            raise RuntimeError('No GRID process observed')
        target = pids[0]
        result['target_pid'] = target
        send(f'attach {target:X}')
        wait_for('[STATE] paused', 15)
        send('log "GRID_DEBUG_ATTACHED eip={p:cip}"')
        attached = wait_for('GRID_DEBUG_ATTACHED', 12)
        result['attach_marker'] = attached
        result['attached'] = bool(attached and '00000000' not in attached)
        # Hardware execution breakpoint survives loader writes to protected .text.
        send('bphws 49FD2D, x, 1')
        hit = None
        for _ in range(8):
            send('run')
            if not wait_for('[STATE] paused', 15):
                break
            send('log "GRID_DEBUG_STOP eip={p:cip}"')
            stopped = wait_for('GRID_DEBUG_STOP', 3)
            if stopped and '0049FD2D' in stopped.upper():
                send('log "GRID_CLEANUP_HIT eip={p:cip} esi={p:esi} field28={p:[esi+28]} field2c={p:[esi+2C]}"')
                hit = wait_for('GRID_CLEANUP_HIT', 3)
                break
            if stopped and '00C01796' in stopped.upper():
                break
        result['breakpoint_marker'] = hit
        result['breakpoint_hit'] = bool(hit)
        if hit:
            send('log "GRID_DEBUG_OBJECT {mem;80@esi}"')
            send('log "GRID_DEBUG_STACK {mem;100@esp}"')
            wait_for('GRID_DEBUG_STACK', 3)
        send('detach')
        time.sleep(0.5)
        send('exit')
        process.wait(timeout=10)
    finally:
        if process.poll() is None:
            try:
                send('detach')
                send('exit')
                process.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                process.terminate()
                process.wait(timeout=5)
        thread.join(timeout=2)
        (OUT / 'x32dbg-startup-early.log').write_text(''.join(lines), encoding='utf-8')
        result['debugger_exit_code'] = process.returncode
        (OUT / 'x32dbg-probe-early.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps(result, indent=2))
    for line in lines:
        if 'GRID_' in line or '[FAIL]' in line or 'attach' in line.lower():
            print(line.rstrip())


if __name__ == '__main__':
    main()
