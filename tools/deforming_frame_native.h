/* Owned context for the unchanged active bulk callback in whole B7B850. */
#include "isolated_imports.h"
static GridBinding active_bindings[]={ {0xc89148,"GetLastError",0,NULL},{0xc890d4,"SetLastError",0,NULL},
 {0xc8912c,"TlsGetValue",0,NULL},{0xc89130,"TlsSetValue",0,NULL},{0xc8911c,"QueryPerformanceFrequency",0,NULL},
 {0xc89218,"QueryPerformanceCounter",0,NULL},{0xc8925c,"GetTickCount",0,NULL} };
static DWORD active_ptd=TLS_OUT_OF_INDEXES,active_cache=TLS_OUT_OF_INDEXES;
typedef struct {
 __declspec(align(16)) unsigned char owner[32+0xd230+32];
 __declspec(align(16)) unsigned char settings[32+0x1054+32];
 unsigned char owner_before[0xd230],settings_before[0x1054],ptd[0x214],wrapper[16],parameters[156],correction[20];
} ActiveFrame;
static int active_initialize(unsigned char *image){
 if(!grid_imports(image,active_bindings,7,0))return 0;
 active_ptd=TlsAlloc();active_cache=TlsAlloc();return active_ptd!=TLS_OUT_OF_INDEXES&&active_cache!=TLS_OUT_OF_INDEXES;
}
static int active_restore(unsigned char *image){
 int okay=1;if(active_ptd!=TLS_OUT_OF_INDEXES){if(!TlsSetValue(active_ptd,NULL)||!TlsFree(active_ptd))okay=0;}
 if(active_cache!=TLS_OUT_OF_INDEXES){if(!TlsSetValue(active_cache,NULL)||!TlsFree(active_cache))okay=0;}
 if(!grid_imports(image,active_bindings,7,1))okay=0;return okay;
}
static int active_prepare(ActiveFrame *a,FILE *file,unsigned char *image,unsigned char **descriptor,unsigned char *linked,const float *cap,unsigned disable){
 memset(a,0,sizeof(*a));memset(a->owner,0xcd,sizeof(a->owner));memset(a->settings,0xcd,sizeof(a->settings));
 unsigned char *owner=a->owner+32,*settings=a->settings+32,*p=a->parameters;
 if(!read(file,owner,0xd230)||!read(file,settings,0x1054)||!read(file,p,156)||!read(file,a->correction,20)||get(a->correction,0)>32||!finite_words(a->correction+4,4)||memcmp(owner+0x2300,*descriptor,0xa500)||get(owner,0xdb0)>1||get(owner,0xc7f8))return 0;
 float numbers[39];memcpy(numbers,p,156);
 for(unsigned i=0;i<39;i++)if(i!=14&&i!=15&&i!=18&&i!=37&&i!=38&&!isfinite(numbers[i]))return 0;
 if(get(p,56)<1||get(p,56)>64||get(p,60)>1||get(p,72)>1||get(p,148)>1||get(p,148)!=disable||get(p,0)!=*(const unsigned *)(cap+2)||get(p,4)!=*(const unsigned *)(cap+5)||get(p,80)!=*(const unsigned *)(cap+6)||memcmp(p+84,cap+7,64))return 0;
 if(!finite_words(owner+0x290,10)||!finite_words(owner+0x2e0,1)||!finite_words(owner+0x15a8,1)||*(float *)(owner+0x15a8)<=0||!finite_words(owner+0x1630,4)||!finite_words(owner+0xb20,4)||!finite_words(owner+0xd16c,1)||*(float *)(owner+0xd16c)<=0||!finite_words(owner+0xd22c,1)||*(float *)(owner+0xd22c)<=0||numbers[8]<=0||!finite_words(settings+0x1024,11))return 0;
 unsigned char *d=owner+0x2300;unsigned edges=get(d,0xa410);if(edges>1200||d[0xa4af]>1||get(d,0xa4a0)>2||get(d,0xa450)>64)return 0;
 for(unsigned i=0;i<400;i++)if(!finite_words(d+0x2580+i*80,17))return 0;
 for(unsigned i=0;i<edges;i++)if((get(d,i*8)&65535)>=400||(get(d,i*8)>>16)>=400||!finite_words(d+i*8+4,1))return 0;
 memcpy(a->settings_before,settings,0x1054);memcpy(a->owner_before,owner,0xd230);
 put(owner,0xdb0,(unsigned)(uintptr_t)a->wrapper);put(a->wrapper,8,(unsigned)(uintptr_t)linked);put(d,0xa4f8,(unsigned)(uintptr_t)settings);
 put(image,0xe85f28-0x400000,(unsigned)(uintptr_t)owner);put(image,0xe85fcc-0x400000,1);
 memcpy(image+0xe500a4-0x400000,p+24,20);image[0xe500b8-0x400000]=(unsigned char)get(p,60);
 memcpy(image+0xe500c0-0x400000,p+68,4);put(image,0xe500c4-0x400000,get(p,72));
 memcpy(image+0xe500c8-0x400000,p+8,4);memcpy(image+0xe500cc-0x400000,p+20,4);memcpy(image+0xe500d0-0x400000,p+12,8);
 memcpy(image+0xe4fc38-0x400000,p+64,4);memcpy(image+0xd1676c-0x400000,p+76,4);
 memcpy(image+0xe4fc88-0x400000,p+44,4);put(image,0xe4fc8c-0x400000,get(p,56));memcpy(image+0xe4fc90-0x400000,p+48,8);
 put(image,0xe56968-0x400000,active_ptd);put(image,0xe5696c-0x400000,active_cache);put(a->ptd,0x14,get(p,152));
 if(!TlsSetValue(active_ptd,a->ptd)||!TlsSetValue(active_cache,(void *)(uintptr_t)active_bindings[2].resolved))return 0;
 *descriptor=d;return 1;
}
static int active_finish(ActiveFrame *a){
 unsigned char *owner=a->owner+32;
 if(get(owner,0xdb0)!=(unsigned)(uintptr_t)a->wrapper||get(owner,0xc7f8)!=(unsigned)(uintptr_t)(a->settings+32)||memcmp(a->settings+32,a->settings_before,0x1054))return 0;
 for(unsigned i=0;i<32;i++)if(a->owner[i]!=0xcd||a->owner[32+0xd230+i]!=0xcd||a->settings[i]!=0xcd||a->settings[32+0x1054+i]!=0xcd)return 0;
 put(owner,0xdb0,get(a->owner_before,0xdb0));put(owner,0xc7f8,0);
 for(unsigned i=0;i<0xd230;i++)if(!((i>=0x2300&&i<0xc800)||(i>=0x290&&i<0x2b8)||(i>=0x2e0&&i<0x2e4))&&owner[i]!=a->owner_before[i])return 0;
 return 1;
}
