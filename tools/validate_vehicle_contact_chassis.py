"""Source contact/control/force/assist update through local chassis publication."""
import validate_vehicle_contact_force as base
import validate_vehicle_contact_assists as assists
import validate_vehicle_chassis as chassis

def cases():
    seeds=chassis.cases();result=[]
    for i,record in enumerate(assists.cases()):
        r=bytearray(record);seed=seeds[i]
        for start,end in [(0x50,0x24c),(0x13d0,0x13d8),(0x1494,0x14d0),(0x1630,0x1640)]:
            r[base.BODY+start:base.BODY+end]=seed[4+start:4+end]
        result.append(bytes(r))
    return result

if __name__=='__main__':
    base.MAGIC=b'MF01';base.GRIP_TAG=b'VU01';base.GRIP_NATIVE=base.ROOT/'local/behavior/vehicle-contact-update-probe.exe'
    base.POST_FORCE=True;base.POST_CHASSIS=True;base.cases=cases;base.main()
