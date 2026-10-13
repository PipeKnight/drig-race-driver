/* Bounded selected 9F3E80 input; owned component mode0. */
#define MUTABLE 0xcdf0
static float value(const void *p,unsigned o){float f;memcpy(&f,(const char *)p+o,4);return f;}
static int bounded(const void *p,unsigned start,unsigned end){for(unsigned i=start;i<end;i+=4){float f=value(p,i);if(!isfinite(f)||fabsf(f)>1000000.f)return 0;}return 1;}
/* Installed Standard HDM's force divisor exceeds ten million. These two
   finite compare/divide operands retain the active positive-divisor guard. */
static int handling_parameters(const void *p){return bounded(p,0x218,0x2f0)&&isfinite(value(p,0x2f0))&&isfinite(value(p,0x2f4))&&bounded(p,0x2f8,0x37c);}
#if defined(GRID_HANDLING_QUERIES) && !defined(GRID_HANDLING_TERMINAL)
#include <xmmintrin.h>
/* Guard only: exact original SSE root/refinement order, BEFORE level clamp. */
static int notification_inactive(const unsigned char *v,const unsigned char *b){
    float limit=value(b,0x280);if(limit==0)return 1;
    float threshold=value(b,0x288)*.01f;
    for(unsigned i=0;i<4;i++){
        float angular=fabsf(value(v,0x1790+i*4))/(limit*.01745329238474369f),x[4];
        for(unsigned j=0;j<4;j++){x[j]=value(v,0x1650+i*16+j*4);if(v[0xc7ae])x[j]-=value(v,0xeefc)*value(v,0xc740+j*4);}
        float square=(x[1]*x[1]+x[0]*x[0])+(x[3]*x[3]+x[2]*x[2]),length=0;
        if(square!=0){float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(square))),root=seed*square;length=(1.f-root*seed)*(root*.5f)+root;}
        float displacement=length/value(b,0x284),ratio=angular>displacement?angular:displacement;
        if(!isfinite(angular)||!isfinite(displacement)||ratio>threshold)return 0;
    }
    return 1;
}
#endif
static unsigned handling_guard(const unsigned char *v,const unsigned char *physical,float delta){
    const unsigned char *b=v+MUTABLE;
    if(!isfinite(delta)||delta<0||delta>.1f)return 30;
    if(get(v,0xdb0)||get(b,0x214)||get(v,0x1620)!=4||v[0xb13]!=1||v[0xb14]!=1||get(v,0x22e4)||b[0x442]>
#ifdef GRID_HANDLING_QUERIES
    1
#else
    0
#endif
    ||(get(b,0x444)!=7
#ifdef GRID_HANDLING_TERMINAL
    &&get(b,0x444)!=3
#endif
    )||b[0x440]>1||v[0xc7ae]>1||get(v,0x2118)>1||get(v,0x211c)>2)return 31;
#ifdef GRID_HANDLING_TERMINAL
    if(get(b,0x444)==3&&b[0x448]>3)return 31;
#endif
    if(!bounded(b,0,0x60)||!handling_parameters(b)||!bounded(b,0x3a0,0x3a4)||!bounded(b,0x42c,0x43c)||!bounded(physical,0x1d0,0x230)||!bounded(physical,0x3f0,0x3f4)||value(physical,0x3f0)<=0||value(b,0x368)<0||value(b,0x48)<0||value(b,0x48)>1||value(b,0x4c)<0||value(b,0x4c)>1||value(b,0x280)<0||(value(b,0x280)!=0&&value(b,0x284)<=0)||!bounded(v,0x1650,0x1690)||!bounded(v,0x1790,0x17a0)||!bounded(v,0xc740,0xc750)||!bounded(v,0xeefc,0xef00)||!bounded(v,0x18c4,0x18c8))return 32;
    if(delta>.02f)delta=.02f;
    float threshold=(float)((double)value(physical,0x3f0)*(double)value(b,0x368));
    for(unsigned i=0;i<4;i++)if(!bounded(v,0x1aa8+i*0x150,0x1aac+i*0x150)||!isfinite(value(v,0x1ac8+i*0x150))||
#ifndef GRID_HANDLING_DAMAGE
    value(v,0x1ac8+i*0x150)*delta>threshold||
#endif
    !bounded(b,0x60+i*0x30,0x70+i*0x30)||(value(v,0x1aa8+i*0x150)>value(b,0x2f0)&&value(b,0x2f4)<=0))return 33;
#ifdef GRID_HANDLING_DAMAGE
    (void)threshold;
#endif
#if defined(GRID_HANDLING_QUERIES) && !defined(GRID_HANDLING_TERMINAL)
    if(b[0x442]&&!notification_inactive(v,b))return 34;
#endif
    return 0;
}
