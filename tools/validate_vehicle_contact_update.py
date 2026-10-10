"""Actual force-vector boundary through source suspension/grip and wheel forces."""
import struct
import validate_vehicle_contact_force as base

original_cases=base.cases
original_guards=base.guards

def cases():
    result=[]
    for i,r in enumerate(original_cases()):
        r=bytearray(r)
        # Prior force vectors are overwritten at this earlier start boundary.
        if i%3==0:r[base.VIEW+0x190:base.VIEW+0x290]=struct.pack('<64I',*([0x7fc00000]*64))
        for o,x in [(0x3d0,[.2,.99999,1,1.00001,1.7][i%5])]:struct.pack_into('<f',r,base.BODY+o,x)
        result.append(bytes(r))
    return result

if __name__=='__main__':
    base.MAGIC=b'UF01';base.GRIP_TAG=b'VU01';base.GRIP_NATIVE=base.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    base.cases=cases
    base.main()
