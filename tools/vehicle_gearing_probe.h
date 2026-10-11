/* Whole original 9AD6B0; bounded curve scan and owned driven-wheel pointers. */
static unsigned gearing_guard(const unsigned char *body){
    const float rpm=*(const float *)(body+0x17a4);
    if(!isfinite(rpm)||rpm<0||rpm>20000||!isfinite(*(const float *)(body+0x2218)))return 70;
    for(unsigned i=0;i<32;i++)if(!isfinite(*(const float *)(body+0x17ac+i*4)))return 70;
    for(unsigned i=0;i<10;i++)if(!isfinite(*(const float *)(body+0x18f4+i*4)))return 70;
    for(unsigned i=1;i<10;i++)if(!isfinite(*(const float *)(body+0x191c+i*4))||!isfinite(*(const float *)(body+0x1944+i*4)))return 70;
    const unsigned n=get(body,0x18f0);
    for(unsigned i=0;i<n;i++){
        const float radius=*(const float *)((uintptr_t)get(body,0x18d8+i*4)+0xe0);
        if(!isfinite(radius)||radius<=0)return 70;
    }
    for(unsigned i=1;i<9;i++){
        const float ratio=*(const float *)(body+0x18f4+i*4),following=*(const float *)(body+0x18f8+i*4);
        if(ratio>0&&following>0&&rpm*.9800000190734863f>=1){
            const float factor=1.f/(following/ratio);
            if(!isfinite(factor)||!isfinite(factor*(rpm*.9800000190734863f)))return 71;
        }
    }
    return 0;
}
static unsigned gearing_run(unsigned char *image,unsigned char *body){
    ((void (__cdecl *)(void *))(image+0x9ad6b0-0x400000))(body);
    for(unsigned i=1;i<10;i++)if(!isfinite(*(float *)(body+0x191c+i*4))||!isfinite(*(float *)(body+0x1944+i*4)))return 72;
    return 0;
}
