"""Test existing x32dbg attach/pause/detach on our own x86 smoke executable."""
import argparse
import hashlib
import json
from pathlib import Path
import queue
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--arm-running', action='store_true')
    parser.add_argument('--hardware', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private evidence only')
    out.mkdir(parents=True, exist_ok=False)
    user = out / 'debugger-user'
    user.mkdir()
    target = ROOT / 'local/behavior/debugger-smoke.exe'
    debugger = ROOT / 'local/tools/x64dbg/release/x32/headless.exe'
    record = {'target_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
              'debugger_sha256': hashlib.sha256(debugger.read_bytes()).hexdigest()}
    game = subprocess.Popen([str(target)] + (['no-exceptions'] if args.arm_running else []), stdout=subprocess.PIPE, text=True,
                            creationflags=subprocess.CREATE_NO_WINDOW)
    entry = int(game.stdout.readline().strip(), 16)
    record['test_entry'] = hex(entry)
    record['target_pid'] = game.pid
    dbg = subprocess.Popen([str(debugger), '-userdir', str(user)], cwd=str(debugger.parent),
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                           text=True, encoding='utf-8', errors='replace', bufsize=1,
                           creationflags=subprocess.CREATE_NO_WINDOW)
    record['debugger_pid'] = dbg.pid
    messages = queue.Queue()
    with (out / 'debugger.log').open('x', encoding='utf-8') as log:
        def receive():
            for line in dbg.stdout:
                log.write(line)
                log.flush()
                messages.put(line)
        thread = threading.Thread(target=receive, daemon=True)
        thread.start()

        def send(command):
            dbg.stdin.write(command + '\n')
            dbg.stdin.flush()

        def wait(marker, seconds):
            until = time.monotonic() + seconds
            while time.monotonic() < until:
                try:
                    line = messages.get(timeout=0.2)
                    if '[FAIL]' in line:
                        raise RuntimeError(line.strip())
                    if marker in line:
                        return line.strip()
                except queue.Empty:
                    if dbg.poll() is not None:
                        raise RuntimeError('Debugger exited')
            raise TimeoutError(marker)
        try:
            wait('entering command loop', 15)
            send(f'attach {game.pid:X}')
            wait('Attached to process!', 20)
            try:
                if args.arm_running:
                    raise TimeoutError('Explicit pause disabled')
                wait('[STATE] paused', 3)
                record['implicit_pause'] = True
            except TimeoutError:
                record['implicit_pause'] = False
                if not args.arm_running:
                    send('pause')
                    wait('[STATE] paused', 15)
            if not args.arm_running:
                send('log "SMOKE_PAUSED ip={p:cip}"')
                record['paused_marker'] = wait('SMOKE_PAUSED ip=', 5)
            record['arm_running'] = args.arm_running
            record['hardware_breakpoints'] = args.hardware
            send(f'bph {entry:X},x,1' if args.hardware else f'bp {entry:X},SMOKE_tick,ss')
            if not args.arm_running:
                send('erun')
                wait('[STATE] running', 5)
            wait('[STATE] paused', 15)
            send('log "SMOKE_TICK ip={p:cip}"')
            record['tick_marker'] = wait('SMOKE_TICK ip=', 5)
            if f'{entry:08X}' not in record['tick_marker'].upper():
                raise RuntimeError('Test breakpoint not reached after passing exceptions')
            if args.hardware:
                send(f'bphc {entry:X}')
            send('detach')
            record['detach_marker'] = wait('Debugging stopped!', 20)
            send('exit')
            dbg.wait(timeout=15)
            thread.join(timeout=3)
            record['target_survives'] = game.poll() is None
            record['debugger_exit'] = dbg.returncode
        except Exception as error:
            record['error'] = repr(error)
            if dbg.poll() is None:
                send('detach')
                try:
                    wait('Debugging stopped!', 20)
                    send('exit')
                    dbg.wait(timeout=15)
                except Exception as cleanup:
                    record['cleanup_error'] = repr(cleanup)
            raise
        finally:
            # This process is our own disposable executable, never GRID.
            if game.poll() is None:
                game.terminate()
                game.wait(timeout=5)
            thread.join(timeout=3)
            with (out / 'validation.json').open('x', encoding='utf-8') as f:
                json.dump(record, f, indent=2)
    print(json.dumps(record, indent=2))


if __name__ == '__main__':
    main()
