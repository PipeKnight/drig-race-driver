"""Source vectors/contact/grip/forces through their actual following assist interval."""
import math
import struct
import validate_vehicle_contact_force as base
import validate_vehicle_contact_update as vectors

def cases():
    result=[]
    for i,r in enumerate(vectors.cases()):
        r=bytearray(r)
        for o,x in [(0x226c,0),(0x2274,0)]:struct.pack_into('<f',r,base.BODY+o,x)
        struct.pack_into('<I',r,base.BODY+0x21f4,i%4)
        r[base.BODY+0x1971]=(i//3)%2;r[base.BODY+0x1f2a]=1
        for j in range(4):struct.pack_into('<f',r,base.BODY+0x1990+j*0x150+0x13c,293.15+(i+j)%30)
        if i%20 in [10,11]:
            c=math.cos(.8);s=math.sin(.8)
            axes=([c,s,0,0],[-s,c,0,0],[0,0,1,0]) if i%20==10 else ([1,0,0,0],[0,c,-s,0],[0,s,c,0])
            for start in [0x15e0,0xb40]:
                for j,row in enumerate(axes):struct.pack_into('<4f',r,base.BODY+start+j*16,*row)
            angular=[0,0,-2,0] if i%20==10 else [2,0,0,0]
            for start in [0x15d0,0xb30]:struct.pack_into('<4f',r,base.BODY+start,*angular)
        result.append(bytes(r))
    return result

if __name__=='__main__':
    base.MAGIC=b'HF01';base.GRIP_TAG=b'VU01';base.GRIP_NATIVE=base.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    base.POST_FORCE=True;base.cases=cases;base.main()
