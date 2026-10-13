"""Reconstruct owned whole-wheel/publication evidence; geometry is a precondition."""
import random
import struct
import validate_deformation_publication as common

BASE_CASES=common.cases
BASE_GUARDS=common.guard_payloads
BASE_CHECK=common.check_answer
EXTRA={4,0xc7f0,0x22e0}|set(range(0x1650,0x17a0,4))|set(range(0x30c,0x3ac,4))

def cases():
    rows,identities=BASE_CASES();rng=random.Random(2008264)
    for index,row in enumerate(rows):
        record=bytearray(row['record']);car=memoryview(record)[4:4+common.CAR]
        car[0xc7f2]=int(index%5!=0)
        for at,value in [(0xeefc,[0.,.001,1.,.8][index%4]),(0xd070,[0.,5.,10.,30.][index%4]),(0xe25c,[.1,10.][index%2]),(0xe260,[.2,10.][index%2])]:struct.pack_into('<f',car,at,value)
        struct.pack_into('<4f',car,0x1630,0.,.2,0.,0.)
        for at,n in [(0x1650,16),(0x1790,4),(0x1690,64),(0x30c,40)]:struct.pack_into('<'+'f'*n,car,at,*[common.layout.f32(rng.uniform(-.5,.5)) for _ in range(n)])
        for i in range(4):
            at=0x1990+i*0x150
            struct.pack_into('<4f',car,at,(-.7 if i%2 else .7),.2,(-1.5 if i<2 else 1.5),0.)
            for offset,value in [(0xe0,.3),(0xf4,.31),(0xf8,.2)]:struct.pack_into('<f',car,at+offset,value)
            car[0x22e0+i]=int((index+i)%7==0)
        row['record']=bytes(record)
    return rows,identities

def request(rows):return b'VD02'+struct.pack('<I',len(rows))+b''.join(r['record'] for r in rows)

def guard_payloads(rows):
    # Case six has both a pending rebuild and active grid/view branches.
    good,bad=BASE_GUARDS([rows[6]]*6)
    bad.pop('pending_rebuild')
    for name,at,value in [('bad_pending',0xc7f0,0x20000),('bad_deflated',0x22e0,2),('nan_radius_cache',0x30c,0x7fc00000),('nan_center',0x1630,0x7fc00000),('nan_scale',0xeefc,0x7fc00000),('nan_limit',0xd070,0x7fc00000),('nan_radius_threshold',0xe25c,0x7fc00000),('zero_current_radius',0x1a70,0),('zero_normal_radius',0x1a84,0),('zero_deflated_radius',0x1a88,0),('nan_wheel_position',0x1990,0x7fc00000),('nan_previous_orientation',0x1690,0x7fc00000),('nan_previous_angle',0x1790,0x7fc00000),('bad_active',0xc7ac,0x200)]:
        b=bytearray(good);struct.pack_into('<I',b,12+at,value);bad[name]=bytes(b)
    return good,bad

def check_answer(row,answer,coverage):
    BASE_CHECK(row,answer,coverage,EXTRA)
    before=row['record'][4:4+common.CAR];after=answer[4:4+common.CAR]
    pending=before[0xc7f2]!=0
    assert after[0xc7f2]==0 and after[4]==(1 if pending else before[4])
    for at,n,name in [(0x1650,64,'displacement'),(0x1790,16,'angles'),(0x1690,256,'orientation'),(0x30c,160,'radius_cache'),(0x22e0,4,'deflated')]:
        changed=before[at:at+n]!=after[at:at+n]
        coverage[name+'_changed']+=changed
        if not pending:assert not changed
    coverage['pending_rebuild']+=pending
    coverage['pending_from_clean_body']+=pending and not before[4]

def configure():
    common.PROBE=common.ROOT/'local/behavior/wheel-deformation-publication-probe.exe'
    common.RUST_BIN='grid-wheel-deformation-publication'
    common.SCOPE='Supplied completed geometry rebuild, original dirty transition plus whole9BB1E0 and wholeA03220; empty owned graphics collection. Not A01960/full source factory/live cadence.'
    common.cases=cases;common.request=request;common.guard_payloads=guard_payloads;common.check_answer=check_answer

if __name__=='__main__':
    configure();common.main()
