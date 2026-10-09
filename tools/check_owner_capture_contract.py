"""Synthetic read-only memory fixtures for the prepared owners capture boundary."""
import struct
from capture_reference_spawn import owner_arguments


def fixture(kind=0, first=0x10000, capacity=32, ret=0xb7872a):
    memory={}
    manager=bytearray(0x13c)
    for offset,value in ((0x98,0x30000),(0xac,1),(0xa0,0x31000),(0xb4,1)):
        struct.pack_into('<I',manager,offset,value)
    a,b=bytearray(0x80),bytearray(0x80)
    struct.pack_into('<2I',a,4,first,0);a[0xc]=1
    struct.pack_into('<2I',b,4,0x11000,0x10000);b[0xc]=kind
    memory.update({0x50000:bytes(manager),0x60000:struct.pack('<3I',ret,0x70000,capacity),
                   0x30000:struct.pack('<I',0x40000),0x31000:struct.pack('<I',0x41000),
                   0x40000:bytes(a),0x41000:bytes(b),
                   0x10418:struct.pack('<3I',1,2,3),0x11418:struct.pack('<3I',0,4,5),0xf5e1a4:b'\x07'})
    def read(address,size):
        value=memory[address]
        if len(value)!=size:raise ValueError('Unexpected synthetic read length')
        return value
    return read


def main():
    for ret in (0xb78559,0xb7872a):
        result=owner_arguments(fixture(ret=ret),0x60000,0xb77770,0,0x50000,0)
        assert result['return_address']==hex(ret) and result['capacity']==32
        assert [o['address'] for o in result['owners']]==['0x10000','0x11000']
        assert [p['count'] for p in result['pools']]==[1,1]
        assert result['alternating_order_before']==7
    rejected=0
    for kwargs in ({'kind':1},{'first':0},{'capacity':1},{'ret':0x123456}):
        try:owner_arguments(fixture(**kwargs),0x60000,0xb77770,0,0x50000,0)
        except ValueError:rejected+=1
        else:raise AssertionError('Unsafe/unverified capture input accepted')
    assert rejected==4
    print('owner_capture_contract_valid=2 rejected=4 live_execution=0')


if __name__=='__main__':main()
