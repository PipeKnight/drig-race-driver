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
    p=argparse.ArgumentParser();p.add_argument('session',type=Path);p.add_argument('output',type=Path);p.add_argument('--ownership',action='store_true');p.add_argument('--handling',action='store_true');a=p.parse_args()
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
        elapsed=started-datetime(1601,1,1,tzinfo=timezone.utc)
        expected=(elapsed.days*86400+elapsed.seconds)*1000000+elapsed.microseconds
        assert abs(creation//10-expected)<=2,'Process identity/start changed'
        rows=[]
        def read(address,length):
            assert 0x10000<=address<=0x7ffe0000 and 0<length<=4096 and address+length<=0x7fff0000
            data=c.create_string_buffer(length);count=c.c_size_t()
            if not k.ReadProcessMemory(h,address,data,length,c.byref(count)) or count.value!=length:raise OSError(c.get_last_error(),'Bounded read failed')
            rows.append(dict(address=hex(address),length=length,hex=data.raw.hex()))
            return data.raw
        def word(address):return struct.unpack('<I',read(address,4))[0]
        for address,length in [(0xcaf400,0x70),(0xa09a80,0x170),(0xa0a290,0x130)]:
            read(address,length)
        ownership=[];worlds=[];handling=[]
        if a.ownership or a.handling:
            count=word(0xe85fcc);assert 1<=count<=40,'Registered car count outside scope'
            cars=struct.unpack(f'<{count}I',read(0xe85f28,count*4));assert len(set(cars))==count
            for body in cars:
                wrapper=word(body+0xdb0);physical=word(wrapper+8)
                mutable=read(body+0xcdf0,0x460);component=struct.unpack_from('<I',mutable,0x214)[0]
                assert component and word(component+0x24)==body,'Mutable component/body identity differs'
                header=read(component,0x100);linked=read(physical,0x424);flags=read(body+0xb10,8)
                if a.handling:
                    wheels=read(body+0x1990,0x540)
                    terminal=read(body+0x22e4,4)
                    def f32(data,offset):return struct.unpack_from('<f',data,offset)[0]
                    handling.append(dict(body=hex(body),component_query=hex(struct.unpack_from('<I',header,0x10)[0]),mode=struct.unpack_from('<I',header,0x6c)[0],mass_3f0=f32(linked,0x3f0),threshold_368=f32(mutable,0x368),delta_cap=0.019999999552965164,impact_1ac8=[f32(wheels,0x138+i*0x150) for i in range(4)],wear_1aa8=[f32(wheels,0x118+i*0x150) for i in range(4)],wear_threshold_2f0=f32(mutable,0x2f0),rollover_440=mutable[0x440],notification_442=mutable[0x442],status_444=struct.unpack_from('<I',mutable,0x444)[0],terminal_flags=terminal.hex(),pose_y_1e4=f32(linked,0x1e4)))
                ownership.append(dict(body=hex(body),component=hex(component),wrapper=hex(wrapper),physical=hex(physical),component_mode=struct.unpack_from('<I',header,0x6c)[0],body_flags=flags.hex(),physical_world=hex(struct.unpack_from('<I',linked,0x32c)[0]),contact_group=hex(struct.unpack_from('<I',linked,0x18)[0]),active_next=hex(struct.unpack_from('<I',linked,0x300)[0]),update_next=hex(struct.unpack_from('<I',linked,0x318)[0]),mutable_component_exact=True))
            for owner in sorted({int(row['physical_world'],16) for row in ownership}):
                raw=read(owner,0x810)
                def field(offset):return struct.unpack_from('<I',raw,offset)[0]
                count=field(0xc);assert 1<=count<=16,'World update list count outside scope'
                table=struct.unpack(f'<{count}I',read(field(8),count*4))
                lists=[]
                for entry in table:
                    head,backlink=struct.unpack('<II',read(entry,8))
                    assert backlink==owner,'Update list/world ownership differs'
                    lists.append(dict(address=hex(entry),head=hex(head),world=hex(backlink)))
                worlds.append(dict(address=hex(owner),update_lists=lists,manager_count=field(4),solve_async=raw[0x94],advance_async=raw[0x95],active_selector=field(0x60),active_heads=[hex(field(0x3c+i*4)) for i in range(2)],special_body=hex(field(0x28)),contact_groups=hex(field(0x50)),phase=field(0x7c4),delta_bits=hex(field(0x7c8)),delta=struct.unpack_from('<f',raw,0x7c8)[0]))
        result=dict(ownership=ownership,worlds=worlds,handling=handling,atomic=False,utc=datetime.now(timezone.utc).isoformat(),session=session,read_only=True,rows=rows)
        with a.output.open('x') as f:json.dump(result,f,indent=2)
        print(json.dumps(dict(utc=result['utc'],pid=session['Pid'],masks=rows[0]['hex'],ownership=ownership,worlds=worlds,handling=handling,read_only=True)))
    finally:k.CloseHandle(h)
if __name__=='__main__':main()
