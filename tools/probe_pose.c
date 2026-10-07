/* Whole supplied-state pose predictor/rotation builder without stubs.
   No entry/import/TLS execution or live injection; private mapped data only. */
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

typedef void (__fastcall *Predict)(void *,void *,float);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};
    DWORD protections[4];
    for(unsigned i=0;i<4;++i)
        if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protections+i))return 16;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Predict predict=(Predict)(image+0x9bace0-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[25];
        for(unsigned i=0;i<25;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<0.f||values[0]>.25f){error=12;break;}
        __declspec(align(16)) unsigned char body[0x1640]={0};
        memcpy(body+0x15e0,values+1,64);
        memcpy(body+0x15c0,values+17,16);memcpy(body+0x15d0,values+21,16);
        __try {predict(body,NULL,values[0]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned i=0;i<16;++i)fprintf(output,"\t%08x",*(unsigned *)(body+0x15e0+i*4));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    for(unsigned i=0;i<4;++i){DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=17;}
    VirtualFree(image,0,MEM_RELEASE);
    printf("pose_cases=%u error=%u\n",count,error);
    return error;
}
