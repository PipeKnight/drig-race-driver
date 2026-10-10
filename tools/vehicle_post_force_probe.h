/* Actual caller schedule up to, but excluding, chassis-motion publication.
   Whole original dependencies; all recipients refer to the owned body buffer. */
#include <xmmintrin.h>
static float tail_f(const void *body,unsigned o){float x;memcpy(&x,(const unsigned char *)body+o,4);return x;}
static void tail_set(void *body,unsigned o,float x){memcpy((unsigned char *)body+o,&x,4);}
static unsigned tail_guard(const unsigned char *b,float delta){
    const unsigned fields[]={0xe1c,0xe20,0xe74,0x15a4,0x17a4,0x17a8,0x185c,0x1894,0x18fc,0x1974,0x1f00,0x1fa0,0x226c,0x2274};
    if(!isfinite(delta)||delta<=0||delta>.1f||get(b,0x1620)!=4)return 50;
    for(unsigned i=0;i<sizeof(fields)/sizeof(fields[0]);i++)if(!isfinite(tail_f(b,fields[i])))return 50;
    for(unsigned o=0x15c0;o<0x1610;o+=4)if(!isfinite(tail_f(b,o)))return 50;
    for(unsigned i=0;i<4;i++){
        unsigned w=0x1990+i*0x150;
        const unsigned offsets[]={0,0x40,0x138,0x13c};
        for(unsigned j=0;j<4;j++)if(!isfinite(tail_f(b,w+offsets[j])))return 50;
    }
    for(unsigned i=0;i<4;i+=2)if(get(b,0x21f4)&(1u<<(i/2))){
        unsigned w=0x1990+i*0x150;
        float ratio=(tail_f(b,w+0x40)-tail_f(b,w+0x190))/(tail_f(b,w)-tail_f(b,w+0x150));
        if(!isfinite(ratio))return 50;
    }
    float x=tail_f(b,0x15c0),y=tail_f(b,0x15c4),z=tail_f(b,0x15c8),w=tail_f(b,0x15cc);
    if(!isfinite((z*z+w*w)+(x*x+y*y)))return 50;
    return 0;
}
typedef void (__cdecl *TailIdle)(void *,float);
typedef void (__cdecl *TailDisabled)(void *);
typedef void (__fastcall *TailCamber)(void *);
typedef void (__fastcall *TailStep)(void *,void *,float);
static unsigned tail_run(unsigned char *image,unsigned char *body,float delta){
    float x=tail_f(body,0x15c0),y=tail_f(body,0x15c4),z=tail_f(body,0x15c8),w=tail_f(body,0x15cc);
    float square=(z*z+w*w)+(x*x+y*y),speed=0;
    if(square!=0){float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(square)));float product=seed*square;speed=((1-product*seed)*(product*.5f))+product;}
    tail_set(body,0x1648,speed);
    if(tail_f(body,0x1974)<.25f&&!body[0x1970]&&body[0x1971]&&tail_f(body,0x1fa0)>0){
        float low=tail_f(body,0x17a8)*1.01f;if(tail_f(body,0x1894)<low)tail_set(body,0x1894,low);
    }
    if(body[0x1971])((TailIdle)(image+0x990350-0x400000))(body,delta);
    tail_set(body,0x1f00,tail_f(body,0x1894));
    if(tail_f(body,0x226c)!=0)((TailDisabled)(image+0x990220-0x400000))(body);
    ((TailCamber)(image+0x996800-0x400000))(body);
    ((TailStep)(image+0x9b9500-0x400000))(body,NULL,delta);
    ((TailStep)(image+0x9bf230-0x400000))(body,NULL,delta);
    if(tail_f(body,0x2274)!=0)tail_set(body,0x1894,0);
    const unsigned outputs[]={0x1648,0x1894,0x1f00,0xe74,0x15d0,0x15d4,0x15d8,0x15dc};
    for(unsigned i=0;i<8;i++)if(!isfinite(tail_f(body,outputs[i])))return 51;
    for(unsigned i=0;i<4;i++)if(!isfinite(tail_f(body,0x1990+i*0x150+0x13c)))return 51;
    return 0;
}
