/* Conservative whole-outer ownership/setup and shared synchronization guards. */
static int outer_finite(const void *p,unsigned n){for(unsigned i=0;i<n;i++)if(!isfinite(((const float *)p)[i]))return 0;return 1;}
static unsigned outer_sync_guard(const unsigned char *v,const unsigned char *p){
    if(get(v,0xdb0)||get(v,0xc7ec)>=10||get(v,0xc714)||get(v,0xc718)||get(v,0xc71c)||!outer_finite(p+0x1d0,24)||!outer_finite(v+0xc7c4,10)||!outer_finite(v+0xc7b4,4))return 30;
    const unsigned settings[]={0xd198,0xd1ac,0xd1b0,0xce04,0xd1c8,0xd1e0,0xd1f8,0xd210,0xd19c,0xd1c4,0xd1dc,0xd1f4,0xd20c,0xd1bc,0xd1d4,0xd1ec,0xd204,0xd1a8,0xd194,0xd1a0,0xd1a4,0xfa8,0xfac,0xfb0,0xfb4,0x1560};
    for(unsigned i=0;i<sizeof(settings)/sizeof(*settings);i++)if(!outer_finite(v+settings[i],1))return 31;
    const float scale=*(const float *)(v+0x1560),mass=*(const float *)(v+0xfa8)*scale;
    const float a=*(const float *)(v+0xfac)*scale,b=*(const float *)(v+0xfb0)*scale,c=*(const float *)(v+0xfb4)*scale;
    const float determinant=(b*c)*a;
    if(!isfinite(mass)||mass<=0||!isfinite(1.f/mass)||!isfinite(a)||a<=0||!isfinite(b)||b<=0||!isfinite(c)||c<=0||!isnormal(determinant)||determinant<=0)return 32;
    return 0;
}

static unsigned outer_guard(const unsigned char *v,const unsigned char *p){
    unsigned error=outer_sync_guard(v,p);if(error)return error;
    unsigned independent=get(v,0x1554)==0;
    if(get(v,0x1620)!=4||get(v,0xdd4)||v[4]||get(v,0xc7a0)==1||v[0xc7f2]||get(v,0xba0)!=(independent?2u:1u)||v[0x20fb4]!=independent)return 31;
    for(unsigned o=0xdb8;o<0xdc8;o+=4)if(get(v,o))return 31;
    if(!outer_finite(v+0xc798,1)||!outer_finite(v+0xc790,1)||!outer_finite(p+0x230,4)||get(p,0x400)==0xffffffff)return 32;
    const unsigned pointers[]={0x328,0x32c,0x334,0x338,0x33c,0x340};
    for(unsigned i=0;i<6;i++)if(get(p,pointers[i]))return 32;
    return 0;
}
