/* Selected recorded car/static-shape metadata; whole original producer output. */
static Object *wheel_source_objects;static unsigned wheel_source_count;
typedef struct {
    __declspec(align(16)) unsigned char producer[0x940],world[0x20],world_wrapper[16],car_shape[0x280],car_wrapper[16];
    __declspec(align(16)) unsigned slots[4][32];
} WheelOwner;
static unsigned wheel_token(unsigned address,int wrapper){
    if(!address)return 0;
    for(unsigned i=0;i<wheel_source_count;i++)if(address==(unsigned)(uintptr_t)(wrapper?(void *)wheel_source_objects[i].wrapper:(void *)wheel_source_objects[i].shape))return i+2;
    return 0xffffffff;
}
static unsigned wheel_input_guard(const DriverRecord *r){
    if(!wheel_source_objects||!wheel_source_count||wheel_source_count>512||r->producer_capacity>4||r->producer_used>r->producer_capacity||get(r->body,0xdd8))return 41;
    for(unsigned i=0;i<4;i++){unsigned t=get(r->body,0x1a54+i*0x150);if(t&&(t<2||t>wheel_source_count+1))return 41;}return 0;
}
static unsigned wheel_point_guard(const unsigned char *b,const unsigned char *p){
    if(!b[0x1ee0]||!get(b,0xdc8))return 0;
    for(unsigned i=0;i<4;i++){
        unsigned w=0x1990+i*0x150;
        float distance=*(const float *)(b+w+0x40),limit=*(const float *)(b+w+0x50);
        if(b[0x22e4+i]||distance<=limit||!get(b,w+0xc4))continue;
        if(wheel_token(get(b,w+0xc4),1)==0xffffffff)return 41;
        float v[4];memcpy(v,b+w,16);v[1]=(v[1]-*(const float *)(b+w+0xe0))+(distance+limit)*.5f;v[3]=1;
        for(unsigned j=0;j<4;j++){
            float result=(v[3]* *(const float *)(p+0x200+j*4)+v[1]* *(const float *)(p+0x1e0+j*4))+(v[0]* *(const float *)(p+0x1d0+j*4)+v[2]* *(const float *)(p+0x1f0+j*4));
            result-=*(const float *)(p+0x1c0+j*4);
            if(!isfinite(v[j])||!isfinite(result)||!isfinite(*(const float *)(b+0x15f0+j*4)))return 42;
        }
        if(!isfinite(distance-limit))return 42;
    }return 0;
}
static void wheel_setup(WheelOwner *o,const DriverRecord *r,unsigned char *b,unsigned char *p){
    memset(o,0,sizeof(*o));memcpy(o->slots,r->contact_slots,sizeof(o->slots));
    put(o->producer,0x91c,(unsigned)(uintptr_t)o->slots);put(o->producer,0x920,r->producer_capacity);put(o->producer,0x924,r->producer_used);
    put(o->world,0x18,(unsigned)(uintptr_t)o->producer);put(o->world_wrapper,4,(unsigned)(uintptr_t)o->world);
    put(o->car_shape,0xd8,(unsigned)(uintptr_t)p);put(o->car_shape,0x224,0x7f7fffff);put(o->car_shape,0x22c,0x7f7fffff);o->car_shape[0x229]=1;
    put(o->car_wrapper,8,(unsigned)(uintptr_t)o->car_shape);
    put(b,0xdd8,(unsigned)(uintptr_t)o->world_wrapper);if(get(b,0xdc8))put(b,0xdc8,(unsigned)(uintptr_t)o->car_wrapper);
}
static unsigned wheel_output(WheelOwner *o,unsigned char *saved,DriverRecord *r,unsigned char *b,unsigned char *p){
    unsigned used=get(o->producer,0x924);
    put(saved,0x924,used);if(memcmp(o,saved,sizeof(WheelOwner)-sizeof(o->slots)))return 43;
    if(used<r->producer_used||used>r->producer_capacity||get(b,0xdd8)!=(unsigned)(uintptr_t)o->world_wrapper||get(b,0xdc8)!=(get(r->body,0xdc8)?(unsigned)(uintptr_t)o->car_wrapper:0u))return 43;
    for(unsigned i=r->producer_used;i<used;i++){
        unsigned *c=o->slots[i];if(c[1]!=(unsigned)(uintptr_t)p||c[2]||c[0x28/4]!=(unsigned)(uintptr_t)o->car_shape||c[0x30/4]||c[0x34/4])return 43;
        unsigned t=wheel_token(c[0x2c/4],0);if(t==0xffffffff)return 43;c[1]=1;c[0x28/4]=1;c[0x2c/4]=t;
    }
    memcpy(r->contact_slots,o->slots,sizeof(o->slots));r->producer_used=used;put(b,0xdd8,0);put(b,0xdc8,get(r->body,0xdc8));return 0;
}
