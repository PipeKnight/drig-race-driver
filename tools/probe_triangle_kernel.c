/* Isolated read-only research harness. Never runs the game's entry point,
   imports, TLS callbacks or constructors. Only the two inspected numeric
   routines execute; original image bytes and outputs remain private. */
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

typedef unsigned char (__cdecl *TriangleTest)(float *,float *,float *,float *,float *,float *,float *,float *);
#include "isolated_reference.h"

int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4) { fputs("usage: triangle-kernel-probe PRIVATE_PE INPUT.tsv PRIVATE_OUTPUT.tsv\n",stderr); return 2; }
    unsigned char *image=map_reference(argv[1]);
    if(!image) { fputs("verified numeric reference mapping failed\n",stderr); return 3; }
    FILE *input=NULL,*output=NULL;
    if(_wfopen_s(&input,argv[2],L"r") || !input) return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE) { fclose(input); return 5; }
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0) { CloseHandle(handle); fclose(input); return 6; }
    output=_fdopen(descriptor,"w");
    if(!output) { _close(descriptor); fclose(input); return 7; }
    unsigned control=0;
    _controlfp_s(&control,0,0);
    fprintf(output,"C\t%08x\t%08x\t%08x\n",_mm_getcsr(),control,(unsigned)(uintptr_t)image);
    TriangleTest test=(TriangleTest)(image+0x6dc290);
    char line[4096],name[64]; unsigned count=0; int result=0;
    while(fgets(line,sizeof(line),input)) {
        __declspec(align(16)) float vectors[5][4]={{0}};
        float fraction=NAN,u=NAN,v=NAN;
        if(++count>100000 || !strchr(line,'\n')) { result=8; break; }
        int read=sscanf_s(line,"%63s %f %f %f %f %f %f %f %f %f %f %f %f %f %f %f",name,(unsigned)sizeof(name),
            &vectors[0][0],&vectors[0][1],&vectors[0][2],&vectors[1][0],&vectors[1][1],&vectors[1][2],
            &vectors[2][0],&vectors[2][1],&vectors[2][2],&vectors[3][0],&vectors[3][1],&vectors[3][2],
            &vectors[4][0],&vectors[4][1],&vectors[4][2]);
        if(read!=16) { result=9; break; }
        for(unsigned a=0;a<5;a++) for(unsigned b=0;b<3;b++) if(!isfinite(vectors[a][b])) result=10;
        if(result) break;
        unsigned char hit=0;
        __try {
            hit=test(vectors[0],vectors[1],vectors[2],vectors[3],vectors[4],&fraction,&u,&v);
        } __except(EXCEPTION_EXECUTE_HANDLER) {
            fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());
            result=13; break;
        }
        if(hit) fprintf(output,"H\t%s\t%08x\t%08x\t%08x\n",name,bits(fraction),bits(u),bits(v));
        else fprintf(output,"N\t%s\n",name);
    }
    if(ferror(input) || ferror(output)) result=11;
    fclose(input);
    if(fclose(output)) result=12;
    VirtualFree(image,0,MEM_RELEASE);
    printf("isolated_cases=%u result=%d\n",count,result);
    return result;
}
