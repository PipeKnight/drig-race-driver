/* Whole Standard preload builder and gravity helper, supplied gravity/geometry.
   x87 53-bit nearest explicitly selected; no stubs or live injection. */
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

typedef void (__cdecl *Initialize)(void *,void *);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    const unsigned pages[]={0xe58000,0xe8e000};DWORD protection[2];
    for(unsigned i=0;i<2;++i)if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protection+i))return 16;
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
    Initialize initialize=(Initialize)(image+0x9ae8f0-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[10];
        for(unsigned i=0;i<10;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<=0.f||values[1]<0.f){error=12;break;}
        __declspec(align(16)) unsigned char record[0x660]={0},body[0x1f00]={0};
        *(unsigned *)(record+0x140)=1;
        memcpy(body+0x15a0,values,4);
        float gravity_y=-values[1];memcpy(image+0xe8efd4-0x400000,&gravity_y,4);
        for(unsigned w=0;w<4;++w){
            memcpy(body+0x1990+w*0x150+8,values+2+w,4);
            memcpy(body+0x1990+w*0x150+0x5c,values+6+w,4);
        }
        __try {initialize(body,record);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned w=0;w<4;++w)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+0x54));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    for(unsigned i=0;i<2;++i){DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protection[i],&ignored))error=18;}
    VirtualFree(image,0,MEM_RELEASE);
    printf("preload_cases=%u error=%u\n",count,error);
    return error;
}
