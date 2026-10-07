/* Isolated supplied-impulse response. No entry/import/TLS or live injection.
   Both original helpers and their cross-product helper run without stubs. */
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

typedef void (__cdecl *Angular)(float *,void *,float *,float *);
typedef void (__cdecl *Apply)(float *,void *,float *);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    unsigned point_response=(argc==5 && !wcscmp(argv[4],L"point"));
    if(argc!=4 && !point_response)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Angular angular=(Angular)(image+0x9cc3d0-0x400000);
    Apply apply=(Apply)(image+0x9cc6e0-0x400000);
    Apply apply_point=(Apply)(image+0x9cc750-0x400000);
    Apply velocity_point=(Apply)(image+0x99c6f0-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[36];
        for(unsigned i=0;i<36;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<=0.f||values[1]<=0.f||values[2]<=0.f||values[3]<=0.f){error=12;break;}
        __declspec(align(16)) unsigned char body[0x80]={0};
        __declspec(align(16)) float response[4]={0};
        memcpy(body,values,4);memcpy(body+8,values+1,12);
        memcpy(body+0x40,values+4,64);memcpy(body+0x20,values+20,16);memcpy(body+0x30,values+24,16);
        __try {
            if(point_response) {
                /* Point velocity takes three cdecl arguments; see its exact ABI. */
                velocity_point(response,body,values+32);
                apply_point(values+28,body,values+32);
            } else {angular(response,body,values+28,values+32);apply(values+28,body,values+32);}
        }
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",bits(response[i]));
        for(unsigned i=0;i<8;++i)fprintf(output,"\t%08x",*(unsigned *)(body+0x20+i*4));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    VirtualFree(image,0,MEM_RELEASE);
    printf("supplied_impulse_cases=%u error=%u\n",count,error);
    return error;
}
