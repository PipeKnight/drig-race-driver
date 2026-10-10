/* Original slowdown/inertia/roughness helpers, actual caller scheduling. */
static unsigned prepared_drag_changes,prepared_roughness_changes,prepared_roughness_rng,prepared_inertia_changes;
static unsigned prepared_guard(const unsigned char *body,const unsigned char material[4][0x140],float delta){
    const unsigned offsets[]={0x1648,0x1f6c,0x1f7c,0x17a8,0x1894,0x15a0,0x15a8,0x15ac,0x15b0};
    for(unsigned i=0;i<sizeof(offsets)/sizeof(*offsets);i++)if(!isfinite(*(float *)(body+offsets[i])))return 50;
    if(*(float *)(body+0x15a0)<=0)return 50;
    for(unsigned i=0;i<3;i++)if(*(float *)(body+0x15a8+i*4)<=0)return 50;
    for(unsigned i=0;i<11;i++)if(!isfinite(*(float *)(body+0x2198+i*4)))return 50;
    if(!(*(float *)(body+0x21b4)< *(float *)(body+0x21b8)&&*(float *)(body+0x21b8)< *(float *)(body+0x21bc)))return 50;
    float roughness=0;
    for(unsigned i=0;i<4;i++){
        if(!isfinite(*(float *)(body+0x22b4+i*4)))return 50;
        if(*(float *)(body+0x1990+i*0x150+0xe0)<=0)return 50;
        if(get(body,0x1990+i*0x150+0xc0)){
            for(unsigned o=0x10;o<=0x20;o+=4)if(!isfinite(*(float *)(material[i]+o)))return 50;
            if(*(float *)(material[i]+0x14)== *(float *)(material[i]+0x18))return 50;
            roughness= *(float *)(material[i]+0x10)+roughness;
        }
    }
    if(roughness>0&&*(float *)(body+0x1648)>8){
        float threshold=((roughness/4.f)*((*(float *)(body+0x1648)-8.f)*(1.f/12.f))*delta)*3072.f;
        if(!isfinite(threshold)||threshold<0||threshold>2000000000.f)return 50;
    }
    return 0;
}
static unsigned prepared_run(unsigned char *image,unsigned char *body,unsigned char *view,Update update,float delta){
    unsigned char motion[32],inertia[12];memcpy(motion,body+0x15c0,32);memcpy(inertia,body+0x15a8,12);
    ((Update)(image+0x9dfc50-0x400000))(body,delta);
    prepared_drag_changes+=memcmp(motion,body+0x15c0,32)!=0;
    for(unsigned i=0;i<8;i++)if(!isfinite(*(float *)(body+0x15c0+i*4)))return 51;
    float speed= *(float *)(body+0x1648),timer= *(float *)(body+0x1f6c);
    if(speed<10||speed>50||(timer!=0&&timer!=1))*(float *)(body+0x1f6c)=fminf(1.f,fmaxf(0.f,(speed>30?.5f:-.5f)*delta+timer));
    timer= *(float *)(body+0x1f7c);
    if(*(float *)(body+0x17a8)*.5f<= *(float *)(body+0x1894))timer-=delta;else timer+=delta;
    *(float *)(body+0x1f7c)=fminf(2.f,fmaxf(0.f,timer));
    unsigned char *solver=(unsigned char *)(uintptr_t)get(view,0);put(solver,0,0);solver[4]=1;put(solver,8,0);put(solver,12,0);
    ((void (__cdecl *)(void *))(image+0x98fca0-0x400000))(body);
    prepared_inertia_changes+=memcmp(inertia,body+0x15a8,12)!=0;
    for(unsigned i=0;i<3;i++)if(!isfinite(*(float *)(body+0x15a8+i*4))||*(float *)(body+0x15a8+i*4)<=0)return 51;
    unsigned seed=get(body,0x1644);memcpy(motion,body+0x15d0,16);
    ((Update)(image+0x9b44b0-0x400000))(body,delta);
    prepared_roughness_rng+=seed!=get(body,0x1644);prepared_roughness_changes+=memcmp(motion,body+0x15d0,16)!=0;
    for(unsigned i=0;i<4;i++)put(view,0x8c+i*4,0);
    return propulsion_run(image,body,view,update,delta);
}
