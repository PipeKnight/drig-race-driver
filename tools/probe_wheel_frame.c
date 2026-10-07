/* Whole four-wheel geometry helper, optional active tyre-axis branch.
   No stubs, code patches, writable mapped pages or live injection. */
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

typedef void (__fastcall *Initialize)(void *, void *);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    int tyre=argc==5&&!wcscmp(argv[4],L"tyre");if(argc!=4&&!tyre)return 2;
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
    Initialize initialize=(Initialize)(image+0x9c13d0-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[60];
        for(unsigned i=0;i<(tyre?60u:52u);++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor){error=12;break;}
        __declspec(align(16)) unsigned char body[0x1f00]={0},context[0x740]={0};
        memcpy(body+0x15e0,values,64);
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;
        *(unsigned *)(context+0x5d8)=4;
        context[0x70c]=(unsigned char)tyre;
        for(unsigned w=0;w<4;++w){
            memcpy(body+0x1990+w*0x150,values+16+w*4,16);
            memcpy(context+0x50+w*0x170,values+32+w*4,16);
            memcpy(context+0x64+w*0x170,values+48+w,4);
            if(tyre)memcpy(body+0x19b0+w*0x150,values+52+w*2,8);
        }
        __try {initialize(context,NULL);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned w=0;w<4;++w){
            for(unsigned j=0;j<12;++j)fprintf(output,"\t%08x",*(unsigned *)(context+0x10+w*0x170+j*4));
            if(tyre)for(unsigned j=0;j<8;++j)fprintf(output,"\t%08x",*(unsigned *)(context+0xc0+w*0x170+j*4));
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    VirtualFree(image,0,MEM_RELEASE);
    printf("wheel_frame_cases=%u error=%u\n",count,error);
    return error;
}
