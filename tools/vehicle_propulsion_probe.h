/* Actual post-control A03670 segment, whole original engine/differential/traction
   helpers and force driver. Scheduling around the calls is independently authored
   from aligned caller instructions; this is not execution of whole A03670. */
typedef float (__cdecl *EngineTorque)(void *,void *,float);
typedef void (__fastcall *Differential)(void *,void *,float);
typedef void (__cdecl *Traction)(void *);
static unsigned propulsion_guard(const unsigned char *body,const unsigned char *view,const unsigned char material[4][0x140]){
    const unsigned engine[]={0,4,8,0xa8,0xb4,0xbc,0xc4,0xc8,0xd0,0xd4,0xdc,0xe0,0xe4,0xe8,0xec,0xf4,0xf8,0xfc,0x100,0x104,0x108,0x10c};
    const unsigned scalars[]={0x1178,0x1f5c,0x1f98,0x1f9c,0x1fa0,0x22c4,0x22c8,0x1570,0x1574,0x1578,0x157c,0x1580,0x1584,0x158c,0x1590,0x1594,0x2264,0x1f70};
    for(unsigned i=0;i<sizeof(engine)/sizeof(*engine);i++)if(!isfinite(*(float *)(body+0x17a0+engine[i])))return 30;
    for(unsigned i=0;i<32;i++)if(!isfinite(*(float *)(body+0x17ac+i*4)))return 30;
    for(unsigned i=0;i<sizeof(scalars)/sizeof(*scalars);i++)if(!isfinite(*(float *)(body+scalars[i])))return 30;
    if(*(float *)(body+0x1f5c)<=0||*(float *)(body+0x1f98)<=0||fabsf(*(float *)(body+0x1894))>1000000||fabsf(*(float *)(body+0x1898))>1000000)return 30;
    if(1.f+(*(float *)(body+0x1fa0)/ *(float *)(body+0x1f98))* *(float *)(body+0x1f9c)==0||(*(float *)(body+0x1590)>0&&*(float *)(body+0x1594)<1))return 30;
    if(get(body,0x1988)>2)return 30;
    float weighted=0;
    for(unsigned i=0;i<4;i++){
        const unsigned w=0x1990+i*0x150;const unsigned scalars_w[]={0x78,0x94,0x98,0x9c,0xa0,0xa4,0xac,0x100,0x104,0x108,0x10c,0x110};
        for(unsigned j=0;j<sizeof(scalars_w)/sizeof(*scalars_w);j++)if(!isfinite(*(float *)(body+w+scalars_w[j])))return 30;
        if(fabsf(*(float *)(body+w+0x104))>1000000||fabsf(*(float *)(body+w+0x110))>1000000)return 30;
        if(get(body,w+0xc0)&&!isfinite(*(float *)(material[i]+0x20)))return 30;
        for(unsigned j=0;j<4;j++)if(!isfinite(*(float *)(view+0x250+i*16+j*4)))return 30;
        weighted+=fabsf(*(float *)(body+w+0x10c));
    }
    if(body[0x1588]&&weighted==0)return 30;
    return 0;
}
static unsigned propulsion_run(unsigned char *image,unsigned char *body,unsigned char *view,Update update,float delta){
#ifdef GRID_CONTROL
    ((Update)(image+0x9b40d0-0x400000))(body,delta);
#endif
    const unsigned gear=get(body,0x196c);float ratio=*(float *)(body+0x18f4+gear*4);if(ratio==0)ratio=1;
    const float torque=((EngineTorque)(image+0x9b1b90-0x400000))(body,body+0x17a0,delta);
    *(float *)(view+0x58)=(float)((double)torque/(double)ratio);
    if(!isfinite(*(float *)(view+0x58)))return 31;
    memcpy(body+0x1f60,view+0x58,4);
    ((Differential)(image+0x996290-0x400000))(body,NULL,delta);
    ((Traction)(image+0x990060-0x400000))(body);
    const float blend=fminf(delta* *(float *)(image+0xcafa58-0x400000),1.f),keep=1.f-blend;
    for(unsigned i=0;i<4;i++){
        unsigned char *w=body+0x1990+i*0x150;
        *(float *)(w+0xac)=keep* *(float *)(w+0xac)+ *(float *)(w+0xa4)*blend;
        *(float *)(w+0x94)=keep* *(float *)(w+0x94)+blend* *(float *)(w+0x9c);
        *(float *)(w+0x98)= *(float *)(w+0xa0)*blend+ *(float *)(w+0x98)*keep;
        put(w,0xa4,0);put(w,0x9c,0);put(w,0xa0,0);
        float angle= *(float *)(w+0x104)*delta+ *(float *)(w+0x110);
        const float pi= *(float *)(image+0xd150c8-0x400000),twopi= *(float *)(image+0xd150c4-0x400000),negativepi= *(float *)(image+0xd150c0-0x400000);
        if(angle>pi)angle-=twopi;if(angle<negativepi)angle+=twopi;*(float *)(w+0x110)=angle;
    }
    __declspec(align(16)) unsigned char points[64];memcpy(points,view+0x250,64);
    for(unsigned i=0;i<4;i++){
        unsigned ptr=get(body,0x1990+i*0x150+0xc0);
        if(ptr){
            const float amount= *(float *)(body+0x1f70)* *(float *)((uintptr_t)ptr+0x20);
            for(unsigned j=0;j<4;j++)*(float *)(view+0x250+i*16+j*4)-=amount* *(float *)(body+0x15f0+j*4);
            put(view,0x250+i*16+12,0x3f800000);
        }
    }
    for(unsigned i=0;i<4;i++)if(!isfinite(*(float *)(view+0x7c+i*4))||!isfinite(*(float *)(view+0x8c+i*4)))return 31;
    if(!isfinite(*(float *)(view+0x9c)))return 31;
    update(body,delta);memcpy(view+0x250,points,64);memcpy(body+0x15a8,view+0x48,12);return 0;
}
