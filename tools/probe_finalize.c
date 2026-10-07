/* Whole two-pass finalizer, four wheels, extra tyre axes inactive, contact
   object pointers null. Original helpers unstubbed; no live injection. */
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

typedef void (__fastcall *Finalize)(void *,void *);
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
    Finalize finalize=(Finalize)(image+0x9e4b10-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[120];
        for(unsigned i=0;i<120;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[28]<=0.f||values[28]>.25f){error=12;break;}
        __declspec(align(16)) unsigned char body[0x1f00]={0},context[0x740]={0};
        unsigned char *state=body+0x15a0;
        memcpy(state,values,4);memcpy(state+8,values+1,12);
        memcpy(state+0x40,values+4,64);memcpy(state+0x20,values+20,16);memcpy(state+0x30,values+24,16);
        *(uint32_t *)(context+0x5d4)=(uint32_t)(uintptr_t)body;
        *(uint32_t *)(context+0x5d8)=4;
        memcpy(context+0x610,values+28,4);memcpy(context+0x6f0,values+29,12);
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=context+w*0x170,*original=body+0x1990+w*0x150;
            float *v=values+32+w*22;
            memcpy(wheel+0x10,v,16);memcpy(wheel+0x30,v+4,16);memcpy(wheel+0x40,v+8,16);
            memcpy(wheel+0x168,v+12,4);memcpy(wheel+0x16c,v+13,4);
            memcpy(wheel+0x174,v+14,4);memcpy(wheel+0x170,v+15,4);
            memcpy(wheel+0x64,v+16,4);memcpy(wheel+0x74,v+17,4);memcpy(wheel+0x68,v+18,4);
            memcpy(original+0x48,v+19,4);memcpy(original+0x4c,v+20,4);memcpy(original+0x138,v+21,4);
        }
        __try {finalize(context,NULL);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        fprintf(output,"\t%08x",*(unsigned *)state);
        for(unsigned i=0;i<3;++i)fprintf(output,"\t%08x",*(unsigned *)(state+8+i*4));
        for(unsigned i=0;i<16;++i)fprintf(output,"\t%08x",*(unsigned *)(state+0x40+i*4));
        for(unsigned i=0;i<8;++i)fprintf(output,"\t%08x",*(unsigned *)(state+0x20+i*4));
        const unsigned fields[]={0x40,0x3c,0x44,0x138};
        const unsigned accums[]={0x168,0x16c,0x174,0x170};
        for(unsigned w=0;w<4;++w){
            for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+fields[i]));
            for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",*(unsigned *)(context+w*0x170+accums[i]));
        }
        fprintf(output,"\t%08x",*(unsigned *)(context+0x610));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    for(unsigned i=0;i<4;++i){DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=17;}
    VirtualFree(image,0,MEM_RELEASE);
    printf("finalizer_cases=%u error=%u\n",count,error);
    return error;
}
