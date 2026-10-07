/* Whole original angular radius selector and original CRT modulo; supplied samples. */
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
#include <float.h>
#include "isolated_reference.h"

typedef double (__fastcall *Initialize)(void *,void *,int);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    unsigned control;
    if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 17;
    printf("x87_control=%08x\n",control);
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Initialize initialize=(Initialize)(image+0x998470-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[12];
        for(unsigned i=0;i<12;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[1]<=0.f||fabsf(values[0])>1000000.f){error=12;break;}
        for(unsigned i=0;i<5;++i)if(fabsf(values[2+i*2])>1000000.f||values[3+i*2]<=0.f){error=12;break;}
        if(error)break;
        __declspec(align(16)) unsigned char body[0x1f00]={0},context[0x740]={0};
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;
        unsigned ordinal=(count-1)%4;
        memcpy(body+0x1990+ordinal*0x150+0x110,values,4);
        memcpy(body+0x1990+ordinal*0x150+0xe0,values+1,4);
        memcpy(body+0x30c+ordinal*40,values+2,40);
        float result;
        __try {result=(float)initialize(context,NULL,(int)ordinal);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s\t%08x",name,bits(result));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    VirtualFree(image,0,MEM_RELEASE);
    printf("wheel_radius_cases=%u error=%u\n",count,error);
    return error;
}
