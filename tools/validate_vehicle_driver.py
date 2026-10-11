"""Selected earlier driver joined to source contact/force/assist/chassis execution."""
import struct
import validate_vehicle_contact_force as base
import validate_vehicle_contact_chassis as source
import validate_vehicle_driver_prepare as prepare

def cases():
    seeds=prepare.cases();result=[]
    for i,raw in enumerate(source.cases()):
        r=bytearray(raw);seed=seeds[i]
        for start,end in [(0x24a,0x24b),(0x2e0,0x304),(0x3ac,0x3c4),(0xb10,0xb11),(0xde4,0xde5),(0xdfc,0xdfd),
                          (0xe08,0xe18),(0x1108,0x1110),(0x139c,0x13a0),(0x1568,0x156c),(0x1598,0x15a0),
                          (0x1640,0x1641),(0x1850,0x1854),(0x1890,0x1894),(0x1ee4,0x1ef4),(0x1f04,0x1f20),
                          (0x1f34,0x1f38),(0x1f48,0x1f5c),(0x1f64,0x1f6c),(0x1f74,0x1f7a),(0x1f94,0x1f98),
                          (0x21c4,0x21c8),(0x21f0,0x21f1),(0x21f2,0x21f3),(0x2214,0x2218),(0x2268,0x226c)]:
            r[base.BODY+start:base.BODY+end]=seed[prepare.BODY+start:prepare.BODY+end]
        r[base.BODY+0x1970:base.BODY+0x1972]=b'\x01\x01'
        struct.pack_into('<I',r,base.BODY+0x196c,1+i%6)
        for j in range(4):
            for o in [0x78,0x12c,0x130]:
                a=base.BODY+0x1990+j*0x150+o;b=prepare.BODY+0x1990+j*0x150+o;r[a:a+4]=seed[b:b+4]
        result.append(bytes(r))
    # Both are overwritten by the new earlier interval before their later readers.
    for raw in result[:10]:
        for offset,value in [(base.BODY+0x1648,0xbf800000),(base.VIEW+0x298,0x7fc00000)]:
            r=bytearray(raw);struct.pack_into('<I',r,offset,value);result.append(bytes(r))
    return result

if __name__=='__main__':
    base.MAGIC=b'EF01';base.GRIP_TAG=b'VU01';base.GRIP_NATIVE=base.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    base.DRIVER_PREPARE=True;base.POST_FORCE=True;base.POST_CHASSIS=True;base.cases=cases;base.main()
