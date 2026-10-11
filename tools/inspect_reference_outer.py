"""Bounded read-only comparison of outer wrapper code/masks in the owned reference."""
import argparse
import ctypes as c
from ctypes import wintypes as w
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
ROOT=Path(__file__).resolve().parents[1]
GAME=Path(r'C:\Users\debros\Tools\grid-research\grid-compat\grid.exe')
HASH='b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6'

def main():
    p=argparse.ArgumentParser();p.add_argument('session',type=Path);p.add_argument('output',type=Path);a=p.parse_args()
    assert a.session.resolve().is_relative_to((ROOT/'local').resolve()) and a.output.resolve().is_relative_to((ROOT/'local').resolve())
    session=json.loads(a.session.read_text());assert Path(session['Path'])==GAME and session['ExeSHA256']==HASH
    assert hashlib.sha256(GAME.read_bytes()).hexdigest()==HASH
    k=c.WinDLL('kernel32',use_last_error=True)
    k.OpenProcess.argtypes=[w.DWORD,w.BOOL,w.DWORD];k.OpenProcess.restype=w.HANDLE
    k.ReadProcessMemory.argtypes=[w.HANDLE,c.c_void_p,c.c_void_p,c.c_size_t,c.POINTER(c.c_size_t)]
    k.CloseHandle.argtypes=[w.HANDLE]
    k.QueryFullProcessImageNameW.argtypes=[w.HANDLE,w.DWORD,w.LPWSTR,c.POINTER(w.DWORD)]
    k.GetProcessTimes.argtypes=[w.HANDLE,c.POINTER(w.FILETIME),c.POINTER(w.FILETIME),c.POINTER(w.FILETIME),c.POINTER(w.FILETIME)]
    h=k.OpenProcess(0x410,False,int(session['Pid']))
    if not h:raise OSError(c.get_last_error(),'Read-only process access failed')
    try:
        path=c.create_unicode_buffer(32768);size=w.DWORD(len(path));assert k.QueryFullProcessImageNameW(h,0,path,c.byref(size)) and Path(path.value)==GAME
        times=[w.FILETIME() for _ in range(4)];assert k.GetProcessTimes(h,*[c.byref(t) for t in times])
        creation=(times[0].dwHighDateTime<<32)|times[0].dwLowDateTime
        started=datetime.fromisoformat(session['StartedUtc'].replace('Z','+00:00'))
        # Python's datetime truncates the original 100ns timestamp to microseconds.
        expected=int((started-datetime(1601,1,1,tzinfo=timezone.utc)).total_seconds()*1000000)
        assert abs(creation//10-expected)<=2,'Process identity/start changed'
        rows=[]
        for address,length in [(0xcaf400,0x70),(0xa09a80,0x170),(0xa0a290,0x130)]:
            data=c.create_string_buffer(length);read=c.c_size_t()
            if not k.ReadProcessMemory(h,address,data,length,c.byref(read)) or read.value!=length:raise OSError(c.get_last_error(),'Bounded read failed')
            rows.append(dict(address=hex(address),length=length,hex=data.raw.hex()))
        result=dict(utc=datetime.now(timezone.utc).isoformat(),session=session,read_only=True,rows=rows)
        with a.output.open('x') as f:json.dump(result,f,indent=2)
        print(json.dumps(dict(utc=result['utc'],pid=session['Pid'],masks=rows[0]['hex'],read_only=True)))
    finally:k.CloseHandle(h)
if __name__=='__main__':main()
