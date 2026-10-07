/* Isolated marker arithmetic; engine calls use authored controlled stubs.
   No entry point, live injection, imports, TLS or random branch is executed. */
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

static float extents[3], output[5];
static unsigned calls;
static void __fastcall dimensions(void *body, void *unused, float *front, float *rear, float *width) {
    (void)body; (void)unused; *front=extents[0]; *rear=extents[1]; *width=extents[2];
}
static float __fastcall speed(void *context, void *unused, void *vehicle, void *point) {
    (void)context; (void)unused; (void)vehicle; (void)point; return 0.f;
}
static void __fastcall flag(void *vehicle, void *unused, unsigned value) {
    (void)vehicle; (void)unused; (void)value;
}
static void __fastcall invalidate(void *vehicle, void *unused) { (void)vehicle; (void)unused; }
static void __cdecl placement(void *body, float x, float z, float *heading, float velocity, float upper) {
    (void)body; (void)velocity; (void)upper; output[0]=x; output[1]=z; memcpy(output+2,heading,12); ++calls;
}
static int patch(unsigned char *image, unsigned address, uintptr_t target) {
    DWORD prior, ignored;
    unsigned char *entry=image+address-0x400000;
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&prior)) return 0;
    entry[0]=0xe9;
    uint32_t displacement=(uint32_t)(uintptr_t)target-(uint32_t)(uintptr_t)(entry+5);
    memcpy(entry+1,&displacement,4);
    return VirtualProtect(entry,5,prior,&ignored) && FlushInstructionCache(GetCurrentProcess(),entry,5);
}
/* Fastcall's unused EDX models the inspected thiscall ECX + four stack args. */
typedef unsigned char (__fastcall *Align)(void *,void *,void *,void *,unsigned,void *);
int wmain(int argc, wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4) return 2;
    unsigned char *image=map_reference(argv[1]);
    DWORD ignored;
    if(!image || !VirtualProtect(image+(0xe58000-0x400000),4096,PAGE_READWRITE,&ignored)) return 3;
    if(!patch(image,0x9b8750,(uintptr_t)&dimensions) || !patch(image,0x915170,(uintptr_t)&speed) || !patch(image,0x9de090,(uintptr_t)&flag) || !patch(image,0x9ad330,(uintptr_t)&invalidate) || !patch(image,0x9ef040,(uintptr_t)&placement)) return 4;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r") || !input) return 5;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE) { fclose(input); return 6; }
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0) { CloseHandle(handle); fclose(input); return 7; }
    FILE *result=_fdopen(descriptor,"w");
    if(!result) { _close(descriptor); fclose(input); return 8; }
    Align align=(Align)(image+0x915270-0x400000);
    char line[4096],name[64]; unsigned count=0, error=0;
    while(fgets(line,sizeof(line),input)) {
        float values[11]; char *cursor=line;
        if(++count>1000 || !strchr(line,'\n')) {error=9;break;}
        char *start=cursor;
        while(*cursor && *cursor!='\t' && *cursor!=' ') ++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length || length>=sizeof(name)) {error=10;break;}
        memcpy(name,start,length);name[length]=0;
        for(unsigned i=0;i<11;++i) {
            char *stop; values[i]=strtof(cursor,&stop);
            if(stop==cursor || !isfinite(values[i])) {error=11;break;}
            cursor=stop;
        }
        while(*cursor=='\r'||*cursor=='\n'||*cursor=='\t'||*cursor==' ') ++cursor;
        if(error || *cursor) {error=11;break;}
        __declspec(align(16)) unsigned char marker[0x60]={0}, vehicle[0x30]={0}, body[16]={0};
        for(unsigned i=0;i<3;++i) { memcpy(marker+0x20+i*16,values+i*3,12); *(float *)(marker+0x2c+i*16)=1.f; }
        *(uint32_t *)(vehicle+0x24)=(uint32_t)(uintptr_t)body;
        extents[0]=values[9];extents[1]=-2.f;extents[2]=values[10];
        float dx=values[3]-values[0],dy=values[4]-values[1],dz=values[5]-values[2];
        if(sqrtf(dx*dx+dy*dy+dz*dz)*.5f>extents[2]) {error=12;break;}
        calls=0;
        __try {
            unsigned char ok=align(marker,NULL,vehicle,NULL,1,NULL);
            if(!ok || calls!=1) {error=13;break;}
            fprintf(result,"P\t%s",name);
            for(unsigned i=0;i<5;++i) fprintf(result,"\t%08x",bits(output[i]));
            fputc('\n',result);
        } __except(EXCEPTION_EXECUTE_HANDLER) {fprintf(result,"F\t%s\t%08lx\n",name,GetExceptionCode());error=14;break;}
    }
    if(ferror(input)||ferror(result)) error=15;
    fclose(input);if(fclose(result)) error=16;
    VirtualFree(image,0,MEM_RELEASE);
    printf("isolated_alignment_cases=%u error=%u\n",count,error);
    return error;
}
