/* Whole setup consumer, supplied endpoints/sliders and inert ancillary state.
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

typedef void (__fastcall *Initialize)(void *, void *);
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
    Initialize initialize=(Initialize)(image+0x9c32f0-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[111];
        for(unsigned i=0;i<111;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[109]<=0.f||values[110]<0.f){error=12;break;}
        __declspec(align(16)) unsigned char body[0xe000]={0};
        __declspec(align(16)) unsigned setup[34]={0};
        setup[0]=(unsigned)(uintptr_t)body;
        memcpy(setup+4,values+60,120);
        memcpy(body+0xeb8,values,120);memcpy(body+0xf30,values+30,120);
        memcpy(body+0x12b8,values+90,8);memcpy(body+0x12c0,values+92,8);
        memcpy(body+0x12c8,values+94,8);memcpy(body+0x1564,values+96,4);
        memcpy(body+0x15a0,values+109,4);
        *(unsigned *)(body+0xfb8)=1; /* Standard decoded arrangement */
        *(unsigned *)(body+0x1620)=4;*(unsigned *)(body+0x18f0)=4;
        for(unsigned j=0;j<6;++j)*(float *)(body+0xd0c4+j*4)=1.f;
        float gravity_y=-values[110];memcpy(image+0xe8efd4-0x400000,&gravity_y,4);
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=body+0x1990+w*0x150;
            memcpy(wheel+0x4c,values+97+w,4);
            memcpy(wheel+4,values+101+w,4);
            memcpy(wheel+8,values+105+w,4);
            *(float *)(wheel+0xe0)=1.f;
            *(unsigned *)(body+0x18d8+w*4)=(unsigned)(uintptr_t)wheel;
        }
        /* Zero power-curve span skips its query in ancillary gearing helper;
           four real dummy wheel pointers/radii make its ratio pass safe. */
        __try {initialize(setup,NULL);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s\t%08x",name,*(unsigned *)(body+0x1564));
        const unsigned offsets[]={0x5c,0x60,0x64,0x68,0x114,0x70,0x4c,4,0x54};
        for(unsigned w=0;w<4;++w)for(unsigned j=0;j<9;++j)
            fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+offsets[j]));
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    for(unsigned i=0;i<2;++i){DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protection[i],&ignored))error=18;}
    VirtualFree(image,0,MEM_RELEASE);
    printf("setup_cases=%u error=%u\n",count,error);
    return error;
}
