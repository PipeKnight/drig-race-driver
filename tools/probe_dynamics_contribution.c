/* Controlled outer dynamics wrapper; internal force/solver calls are stubbed.
   No live injection, entry point, imports, TLS or constructors are executed. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <io.h>
#include <fcntl.h>
#include "isolated_reference.h"

static unsigned calls, first_delta, first_weight, last_weight, inconsistent;
static void __cdecl inertia(void *body) { (void)body; }
static void __fastcall prepare(void *context,void *unused,float delta) {
    (void)context;(void)unused;(void)delta;
}
static void __fastcall finish(void *context,void *unused) { (void)context;(void)unused; }
static void __fastcall substep(void *context,void *unused,float delta,float weight) {
    (void)context;(void)unused;
    if(!calls) { first_delta=bits(delta); first_weight=bits(weight); }
    else if(first_delta!=bits(delta)) inconsistent=1;
    last_weight=bits(weight);++calls;
}
static int redirect(unsigned char *image,unsigned address,uintptr_t target) {
    DWORD prior,ignored; unsigned char *entry=image+address-0x400000;
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&prior))return 0;
    entry[0]=0xe9;
    uint32_t offset=(uint32_t)target-(uint32_t)(uintptr_t)(entry+5);
    memcpy(entry+1,&offset,4);
    return VirtualProtect(entry,5,prior,&ignored)&&FlushInstructionCache(GetCurrentProcess(),entry,5);
}
typedef void (__fastcall *Wrapper)(void *,void *,float);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    if(!redirect(image,0x98fca0,(uintptr_t)&inertia)||!redirect(image,0x9e41f0,(uintptr_t)&prepare)
       ||!redirect(image,0x9e4b10,(uintptr_t)&finish)||!redirect(image,0x9c1f60,(uintptr_t)&substep))return 4;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 5;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 6;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 7;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 8;}
    Wrapper wrapper=(Wrapper)(image+0x9f1e10-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=9;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=10;break;}
        memcpy(name,start,length);name[length]=0;
        float values[22];
        for(unsigned i=0;i<22;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=11;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=12;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<=0.f||values[0]>.25f){error=13;break;}
        __declspec(align(16)) unsigned char body[0x2600]={0},context[0x750]={0};
        *(uint32_t *)(context+0x5d4)=(uint32_t)(uintptr_t)body;
        memcpy(body+0x15c0,values+1,16);memcpy(body+0x15d0,values+5,16);
        memcpy(body+0x1610,values+9,16);memcpy(context+0x620,values+13,16);
        memcpy(context+0x630,values+17,16);memcpy(context+0x610,values+21,4);
        for(unsigned i=0;i<3;++i)*(float *)(body+0x15a8+i*4)=1.f;
        calls=0;inconsistent=0;first_delta=0;first_weight=0;last_weight=0;
        __try {wrapper(context,NULL,values[0]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=14;break;}
        if(calls<1||calls>256||inconsistent){error=15;break;}
        fprintf(output,"S\t%s\t%u\t%08x\t%08x\t%08x\t%08x\n",name,calls,first_delta,
                first_weight,last_weight,*(unsigned *)(context+0x610));
        fprintf(output,"P\t%s",name);
        for(unsigned block=0;block<3;++block) {
            unsigned offset=block==0?0x15c0:block==1?0x15d0:0x1610;
            for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",*(unsigned *)(body+offset+i*4));
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=16;
    fclose(input);if(fclose(output))error=17;
    VirtualFree(image,0,MEM_RELEASE);
    printf("controlled_dynamics_cases=%u error=%u\n",count,error);
    return error;
}
