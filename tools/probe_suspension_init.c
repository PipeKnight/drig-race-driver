/* Whole wheel initializer/reset, supplied decoded coefficients. Original CSV
   decoder not executed. No stubs, entry/import/TLS execution or live injection. */
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

typedef void (__cdecl *Initialize)(void *,void *,void *,unsigned);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
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
    Initialize initialize=(Initialize)(image+0x9b6350-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[18];
        for(unsigned i=0;i<18;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor){error=12;break;}
        __declspec(align(16)) unsigned char record[0x660]={0},wheel[2][0x150]={0},body[0x80]={0};
        const unsigned rear[]={0x410,0x444,0x418,0x420,0x44c,0x454,0x43c,0x3f8,0x400};
        const unsigned front[]={0x40c,0x440,0x414,0x41c,0x448,0x450,0x438,0x3f4,0x3fc};
        for(unsigned i=0;i<9;++i){memcpy(record+rear[i],values+i,4);memcpy(record+front[i],values+9+i,4);}
        __try {initialize(wheel[0],body,record,1);initialize(wheel[1],body,record,0);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        const unsigned fields[]={0x5c,0x60,0x70,0x58,0x64,0x68,0x114,0x4c,0x54,0x48,0x50};
        for(unsigned w=0;w<2;++w)for(unsigned i=0;i<11;++i)
            fprintf(output,"\t%08x",*(unsigned *)(wheel[w]+fields[i]));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    VirtualFree(image,0,MEM_RELEASE);
    printf("suspension_initialization_cases=%u error=%u\n",count,error);
    return error;
}
