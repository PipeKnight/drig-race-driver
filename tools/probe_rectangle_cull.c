/* Whole original horizontal culling helper with supplied cached plane. */
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
#include <wchar.h>
#include "isolated_reference.h"
typedef unsigned (__cdecl *Cull)(float *,float *,float *,float *,float *,float *,float *,float *);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    DWORD old,ignored;if(!VirtualProtect(image+0xec2000-0x400000,0x1000,PAGE_READWRITE,&old))return 4;
    unsigned *cache=(unsigned *)(image+0xec2c54-0x400000);Cull cull=(Cull)(image+0xadb910-0x400000);
    FILE *input=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input)return 5;
    HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)return 6;
    int descriptor=_open_osfhandle((intptr_t)h,_O_TEXT);if(descriptor<0){CloseHandle(h);return 7;}
    FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);return 8;}
    char line[2048],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++count>512||!strchr(line,'\n')){error=9;break;}
        char *cursor=line,*begin=cursor;while(*cursor&&*cursor!='\t')++cursor;size_t n=(size_t)(cursor-begin);if(!n||n>=sizeof(name)){error=10;break;}memcpy(name,begin,n);name[n]=0;
        unsigned words[9];for(unsigned i=0;i<9;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *end;words[i]=strtoul(cursor,&end,16);if(end-cursor!=8){error=11;break;}cursor=end;}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=12;break;}
        float f[8];memcpy(f,words,32);for(unsigned i=0;i<8;++i)if(!isfinite(f[i]))error=13;
        if(error||words[8]>3||f[6]<0||f[7]<0){error=14;break;}*cache=words[8];unsigned hit=0;
        __try{hit=cull(f,f+1,f+2,f+3,f+4,f+5,f+6,f+7)&255;}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=15;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\n",name,hit,*cache);
    }
    if(ferror(input)||ferror(output))error=16;if(fclose(input)||fclose(output))error=17;
    if(!VirtualProtect(image+0xec2000-0x400000,0x1000,old,&ignored))error=18;VirtualFree(image,0,MEM_RELEASE);printf("rectangle_cull_cases=%u error=%u\n",count,error);return error;
}
