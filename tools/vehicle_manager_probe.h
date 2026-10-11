/* Guarded whole A02B70 with original direct dispatch and real OS events. */
#include "isolated_imports.h"
static GridBinding manager_bindings[4]={{0xc89238,"SetEvent",0,NULL},{0xc891a4,"ResetEvent",0,NULL},{0xc89090,"WaitForMultipleObjects",0,NULL},{0xc89148,"GetLastError",0,NULL}};
static HANDLE manager_events[20];
static unsigned manager_bound;
static unsigned manager_open(unsigned char *image){
    if(!grid_imports(image,manager_bindings,4,0))return 36;
    manager_bound=1;
    for(unsigned i=0;i<20;i++){manager_events[i]=CreateEventW(NULL,FALSE,FALSE,NULL);if(!manager_events[i])return 36;}
    return 0;
}
static unsigned manager_close(unsigned char *image){
    unsigned ok=1;for(unsigned i=0;i<20;i++)if(manager_events[i]){if(!CloseHandle(manager_events[i]))ok=0;manager_events[i]=NULL;}
    if(manager_bound&&!grid_imports(image,manager_bindings,4,1))ok=0;manager_bound=0;return ok;
}
static unsigned manager_guard(const unsigned char *b){
    const unsigned counts[]={0xcd8,0xda8,0xc0c,0xd40};
    for(unsigned i=0;i<4;i++)if(get(b,counts[i]))return 37;
    for(unsigned o=0x248;o<0x24c;o++)if(b[o]>1)return 37;
    if((b[0x24b]&&b[0x249]!=(b[0x248]&(b[0x24a]==0)))||get(b,0xdc8)>1||b[0x1ee0]>1)return 37;
    unsigned gear=get(b,0x196c);float speed=*(const float *)(b+0x1648);
    if(gear>=10||!isfinite(speed)||fabsf(speed)>1e10f||!isfinite(*(const float *)(b+0x18f8+4*gear)))return 38;
    if(gear!=9&&*(const float *)(b+0x18f8+4*gear)==0){float rpm=*(const float *)(b+0x1894),limit=*(const float *)(b+0x17a4);if(!isfinite(rpm)||!isfinite(limit)||limit<=0||!isfinite(rpm/limit))return 38;}
    return 0;
}
static unsigned manager_normal(float *v,int square_first){
    float square=(v[1]*v[1]+v[0]*v[0])+(v[3]*v[3]+v[2]*v[2]);if(!isnormal(square)||square<=0)return 39;
    float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(square)));
    float product=square_first?square*seed:seed*square;
    float reciprocal=(1.f-product*seed)*(seed*.5f)+seed;
    for(unsigned i=0;i<4;i++){v[i]=reciprocal*v[i];if(!isfinite(v[i]))return 39;}return 0;
}
static void manager_cross(float *out,const float *a,const float *b){out[0]=a[1]*b[2]-a[2]*b[1];out[1]=b[0]*a[2]-b[2]*a[0];out[2]=b[1]*a[0]-a[1]*b[0];out[3]=0;}
static unsigned manager_finish_guard(const unsigned char *b){
#ifndef GRID_WHEEL_PRODUCER
    if(b[0x1ee0]&&get(b,0xdc8)&&!b[0x22e4])for(unsigned i=0;i<4;i++){unsigned w=0x1990+i*0x150;if(*(const float *)(b+w+0x40)>*(const float *)(b+w+0x50)&&get(b,w+0xc4))return 40;}
#endif
    float forward[4],up[4],side[4];memcpy(forward,b+0x1600,16);
    unsigned error=manager_normal(forward,0);if(error)return error;
    manager_cross(up,forward,(const float *)(b+0x15e0));error=manager_normal(up,0);if(error)return error;
    manager_cross(side,up,forward);return manager_normal(side,1);
}
static void manager_setup(unsigned char *image,unsigned char *body){
    put(image,0xe85f28-0x400000,(unsigned)(uintptr_t)body);put(image,0xe85fcc-0x400000,1);image[0xe85fd8-0x400000]=0;
    memset(image+0xe8ed98-0x400000,0,400);memset(image+0xe8ef28-0x400000,0,84);
    for(unsigned i=0;i<20;i++){unsigned char *j=image+0xe8ed98-0x400000+i*20;put(j,0,(unsigned)(uintptr_t)(image+0xd15438-0x400000));put(j,12,(unsigned)(uintptr_t)(image+0xd316ac-0x400000));put(j,16,(unsigned)(uintptr_t)manager_events[i]);ResetEvent(manager_events[i]);}
}
static unsigned manager_retained(unsigned char *image,unsigned char *body,unsigned delta){
    if(get(image,0xe85f28-0x400000)!=(unsigned)(uintptr_t)body||get(image,0xe85fcc-0x400000)!=1||image[0xe85fd8-0x400000]!=1||get(image,0xe8ef78-0x400000)!=20)return 0;
    for(unsigned i=0;i<20;i++){
        unsigned char *j=image+0xe8ed98-0x400000+i*20;
        if(get(j,0)!=(unsigned)(uintptr_t)(image+0xd15438-0x400000)||get(j,4)!=(i?0u:(unsigned)(uintptr_t)body)||get(j,8)!=(i?0u:delta)||get(j,12)!=(unsigned)(uintptr_t)(image+0xd316ac-0x400000)||get(j,16)!=(unsigned)(uintptr_t)manager_events[i]||get(image,0xe8ef28-0x400000+i*4)!=(unsigned)(uintptr_t)(j+12))return 0;
        // Successful wait-all consumes each auto-reset event.
        if(WaitForSingleObject(manager_events[i],0)!=WAIT_TIMEOUT)return 0;
    }
    return 1;
}
