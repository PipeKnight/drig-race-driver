"""Whole selected control wrapper feeding the actual propulsion/force segment."""
import struct
import itertools
import validate_vehicle_propulsion as base

original_cases=base.cases
original_guards=base.guards


def cases():
    result=[]
    for i,raw in enumerate(original_cases()[6:]):
        b=bytearray(raw)
        def number(o,x):struct.pack_into('<f',b,36+o,x)
        def integer(o,x):struct.pack_into('<I',b,36+o,x&0xffffffff)
        def byte(o,x):b[36+o]=x
        address=struct.unpack_from('<I',b,8)[0]
        pair=list(itertools.permutations(range(4),2))[i%12]
        integer(0x21cc,address+0x1990+pair[0]*0x150);integer(0x21d0,address+0x1990+pair[1]*0x150)
        byte(0x1348,0);integer(0x21ec,i%5);integer(0x18c0,i%5)
        integer(0xe48,10 if i%13 else (i//13)%10);integer(0xe2c,0 if i%5 else [1,4,9,-1][(i//5)%4])
        byte(0xe24,i%17==0);byte(0xe28,i%19==0);byte(0x1970,i%2);byte(0x1971,(i//2)%2);byte(0x1972,(i//4)%2)
        byte(0x18c8,i%7==0);byte(0xe18,i%3==0);byte(0x1640,i%11==0);byte(0x21f2,i%2)
        integer(0x196c,[0,1,2,3,4,5,6,9][i%8]);integer(0x1858,i%3)
        for j,x in enumerate([0,.5,.4,.3,.2,.15,.1,.08,.06,-.5,.04,.02]):number(0x18f4+j*4,x)
        for j in range(10):number(0x191c+j*4,15+j*5);number(0x1944+j*4,10+j*5)
        for o,x in [(0xe1c,[0,.4,1][i%3]),(0xe20,[0,.2,1][(i//3)%3]),(0xe3c,[0,.8][i%2]),(0xe40,[.8,0][i%2]),(0xe34,.5),
                    (0x1974,[0,.2,.8][i%3]),(0x1978,[0,.001,.1,1000][i%4]),(0x1980,[0,1,1.25,2][i%4]),(0x226c,1 if i%23==0 else 0),
                    (0x2280,1 if i%5==0 else 0),(0x22b0,[0,.5,1][i%3]),(0x125c,1),(0x1260,1),(0x1358,.8),(0x1ef4,1),(0x1378,3),
                    (0xe30,[-1,-.2,0,.2,1][i%5]),(0xe54,.4),(0x21d4,1.6),(0x21d8,2.6),(0x2298,1),(0x229c,1),(0x2294,0),
                    (0x1648,[0,.2,1.5,2,3,20,40][i%7]),(0x159c,.2)]:number(o,x)
        byte(0xe6c,(i//3)%2);integer(0x1f74,i%2)
        for o,x in [(0xfdc,.05),(0x138c,.5),(0x123c,.7),(0x1240,0),(0x1244,40),(0xe70,0),(0xe5c,.05),(0xe58,.1),(0x101c,1),(0x1020,20),
                    (0x1024,0),(0x1028,0),(0xfec,.1),(0xfe8,0),(0x1248,0),(0x1250,0),(0x124c,0),(0x1254,40),(0x1388,1),(0x1f2c,.5)]:number(o,x)
        for j in range(4):
            ww=0x1990+j*0x150
            for o,x in [(0x20,1),(0x24,0),(0x28,1),(0x2c,0),(0x38,0),(0xe8,[-.03,0,.03][(i+j)%3]),(0xf0,.9),(0xf4,1),(0xf8,.8),(0x144,[-.05,0,.05][(i+j)%3])]:number(ww+o,x)
            byte(ww+0xfc,j<2);number(0x2284+j*4,1);number(0x22a0+j*4,0);byte(0x22cc+j,(i>>j)&1)
        integer(0x11bc,i%3)
        result.append(bytes(b))
    return result


def guards(out,profile,first):
    # A valid two-driven-wheel fixture with owned shared mechanics exercises alias guards.
    b=bytearray(cases()[1]);body=36;material=36+0x2600+0x300+0xdd0
    b[body+0xe6c]=1
    pointer=struct.unpack_from('<I',b,body+0x1990+0xc0)[0]
    for j in range(1,4):
        struct.pack_into('<I',b,body+0x1990+j*0x150+0xc0,pointer)
        b[material+j*0x140:material+(j+1)*0x140]=b[material:material+0x140]
    extra=[('gear_search',body+0x1348,1,40),('gear_overflow',body+0x196c,10,40),
           ('body_event_count',body+0x21ec,5,40),('engine_event_count',body+0x18c0,5,40),
           ('gear_override',body+0xe48,11,40),('negative_gear_override',body+0xe48,0xffffffff,40),
           ('gear_request',body+0xe2c,10,40),('negative_gear_request',body+0xe2c,0xfffffffe,40),
           ('foreign_steering',body+0x21cc,1,41),('duplicate_steering',body+0x21cc,struct.unpack_from('<I',b,body+0x21d0)[0],41),
           ('zero_steering_span',body+0x21d8,0,40),('zero_brake_sum',body+0x1260,0xbf800000,40),
           ('zero_gear',body+0x18f8,0,40),('zero_wheel_ratio',body+0x1990+0xf4,0,40)]
    for o in [0xe1c,0xe20,0xe34,0xe3c,0xe40,0x1378,0x1974,0x1978,0x1980,0x226c,0x2280,0x22b0,
              0x125c,0x1260,0x1358,0x1ef4,0xe30,0x2294,0x1648,0x159c,0xe54,0x21d4,0x21d8,0x2298,0x229c,
              0xfdc,0x138c,0x123c,0x1240,0x1244,0xe70,0xe5c,0xe58,0x101c,0x1020,0x1024,0x1028,
              0xfec,0xfe8,0x1248,0x1250,0x124c,0x1254,0x1388,0x1f2c]:extra.append((f'control_nan_{o:x}',body+o,0x7fc00000,40))
    return original_guards(out,profile,bytes(b),extra)


if __name__=='__main__':
    base.MAGIC=b'CF01';base.NATIVE=base.ROOT/'local/behavior/vehicle-controls-probe.exe';base.cases=cases
    base.guards=guards
    base.main()
