/* Isolated conditional placement comparison. Only inspected numeric routines
   execute in a private mapped image; ground collection is replaced by our plane
   oracle. No entry point, imports, TLS, live-game injection or input is run. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>
#include <io.h>
#include <fcntl.h>
#include <float.h>
#include <xmmintrin.h>
#include "isolated_reference.h"

typedef unsigned (__cdecl *Placement)(float *,void *,float,float,float *,float);
static float plane[3];
static FILE *evidence;
static char case_name[64];
static unsigned query_count;

static int __cdecl ground(float *start,float *end,float *point,float *fraction,unsigned *material) {
    float height=(plane[0]*start[0]+plane[1]*start[2])+plane[2];
    int hit=height<=start[1] && height>=end[1] && start[0]==end[0] && start[2]==end[2];
    *fraction=hit ? (height-start[1])/(end[1]-start[1]) : NAN;
    memcpy(point,start,16); point[1]=height; *material=0;
    fprintf(evidence,"Q\t%s\t%u",case_name,query_count++);
    for(unsigned i=0;i<3;i++) fprintf(evidence,"\t%08x",bits(start[i]));
    for(unsigned i=0;i<3;i++) fprintf(evidence,"\t%08x",bits(end[i]));
    fprintf(evidence,"\t%d\t%08x\t%08x\n",hit,bits(*fraction),bits(height));
    return hit;
}

static int prepare_oracle(unsigned char *image) {
    DWORD previous;
    unsigned char *entry=image+(0x96b7f0-0x400000);
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&previous)) return 0;
    entry[0]=0xe9;
    uint32_t displacement=(uint32_t)(uintptr_t)&ground-(uint32_t)(uintptr_t)(entry+5);
    memcpy(entry+1,&displacement,4);
    DWORD ignored;
    if(!VirtualProtect(entry,5,previous,&ignored) || !FlushInstructionCache(GetCurrentProcess(),entry,5)) return 0;
    /* The inspected normalizer initializes constants/flags in this data page. */
    return VirtualProtect(image+(0xe58000-0x400000),4096,PAGE_READWRITE,&ignored)!=0;
}

int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4) { fputs("usage: ground-placement-probe PRIVATE_PE INPUT.tsv PRIVATE_OUTPUT.tsv\n",stderr); return 2; }
    unsigned char *image=map_reference(argv[1]);
    if(!image || !prepare_oracle(image)) return 3;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r") || !input) return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE) { fclose(input); return 5; }
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0) { CloseHandle(handle); fclose(input); return 6; }
    evidence=_fdopen(descriptor,"w");
    if(!evidence) { _close(descriptor); fclose(input); return 7; }
    unsigned control=0; _controlfp_s(&control,0,0);
    fprintf(evidence,"C\t%08x\t%08x\t%08x\tcontrolled_plane_queries\n",_mm_getcsr(),control,(unsigned)(uintptr_t)image);
    Placement place=(Placement)(image+(0x9de4e0-0x400000));
    char line[4096]; unsigned count=0; int result=0;
    while(fgets(line,sizeof(line),input)) {
        float fields[29]; char *cursor=line;
        if(++count>10000 || !strchr(line,'\n')) { result=8; break; }
        while(*cursor==' ' || *cursor=='\t') cursor++;
        char *name_start=cursor;
        while(*cursor && *cursor!=' ' && *cursor!='\t' && *cursor!='\n') cursor++;
        size_t name_length=(size_t)(cursor-name_start);
        if(!name_length || name_length>=sizeof(case_name)) { result=9; break; }
        memcpy(case_name,name_start,name_length); case_name[name_length]=0;
        for(unsigned i=0;i<29;i++) {
            char *stop;
            fields[i]=strtof(cursor,&stop);
            if(stop==cursor || !isfinite(fields[i])) { result=10; break; }
            cursor=stop;
        }
        while(*cursor==' ' || *cursor=='\t' || *cursor=='\r' || *cursor=='\n') cursor++;
        if(result || *cursor) { result=10; break; }
        unsigned char *body=_aligned_malloc(0x37100,16);
        unsigned char *wrapper=_aligned_malloc(0x20,16),*state=_aligned_malloc(0x400,16);
        if(!body || !wrapper || !state) { _aligned_free(body); _aligned_free(wrapper); _aligned_free(state); result=11; break; }
        memset(body,0,0x37100); memset(wrapper,0,0x20); memset(state,0,0x400);
        *(uint32_t *)(body+0xdb0)=(uint32_t)(uintptr_t)wrapper;
        *(uint32_t *)(wrapper+8)=(uint32_t)(uintptr_t)state;
        *(unsigned *)(body+0x1620)=4;
        __declspec(align(16)) float pose[16]={1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1};
        memcpy(state+0x1d0,pose,sizeof(pose)); memcpy(body+0x15e0,pose,sizeof(pose));
        __declspec(align(16)) float heading[4]={fields[3],fields[4],fields[5],0};
        memcpy(plane,fields+26,sizeof(plane)); query_count=0;
        for(unsigned i=0;i<4;i++) {
            float *wheel=(float *)(body+0x1990+i*0x150);
            memcpy(wheel,fields+6+i*5,12); wheel[3]=1;
            wheel[0xe0/4]=fields[6+i*5+3]; wheel[0xf4/4]=fields[6+i*5+3];
            wheel[0x40/4]=fields[6+i*5+4];
            if(wheel[0xe0/4]<=0) result=12;
        }
        if(!result) {
            __try {
                place(pose,body,fields[0],fields[2],heading,10000.f);
                fprintf(evidence,"P\t%s",case_name);
                for(unsigned i=0;i<16;i++) fprintf(evidence,"\t%08x",bits(pose[i]));
                fprintf(evidence,"\t%u\n",query_count);
            } __except(EXCEPTION_EXECUTE_HANDLER) {
                fprintf(evidence,"F\t%s\t%08lx\n",case_name,GetExceptionCode()); result=13;
            }
        }
        _aligned_free(body); _aligned_free(wrapper); _aligned_free(state);
        if(result) break;
    }
    if(ferror(input) || ferror(evidence)) result=14;
    fclose(input); if(fclose(evidence)) result=15;
    VirtualFree(image,0,MEM_RELEASE);
    printf("isolated_placement_cases=%u result=%d\n",count,result);
    return result;
}
