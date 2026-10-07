/* Whole original four-wheel preparation, optional active tyre axes; no stubs.
   Supplied body/points/axes. Private mapper only, no live injection. */
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
typedef void (__fastcall *Prepare)(void *,void *);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    int tyre=argc==5&&!wcscmp(argv[4],L"tyre");if(argc!=4&&!tyre)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    /* Only private lazy-init data page becomes writable; code stays RX. */
    DWORD protection;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&protection))return 16;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Prepare prepare=(Prepare)(image+0x9e3970-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[116];
        for(unsigned i=0;i<(tyre?116u:76u);++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<=0.f||values[1]<=0.f||values[2]<=0.f||values[3]<=0.f){error=12;break;}
        __declspec(align(16)) unsigned char body[0x1f00]={0},context[0x740]={0};
        unsigned char *state=body+0x15a0;
        memcpy(state,values,4);memcpy(state+8,values+1,12);
        memcpy(state+0x40,values+4,64);memcpy(state+0x20,values+20,16);memcpy(state+0x30,values+24,16);
        *(uint32_t *)(context+0x5d4)=(uint32_t)(uintptr_t)body;
        *(uint32_t *)(context+0x5d8)=4;
        context[0x70c]=(unsigned char)tyre;
        for(unsigned i=0;i<3;++i)*(float *)(context+0x6f0+i*4)=values[1+i]*2.f;
        for(unsigned i=0;i<4;++i) {
            memcpy(context+i*0x170+0x10,values+28+i*12,16);
            memcpy(context+i*0x170+0x30,values+32+i*12,16);
            memcpy(context+i*0x170+0x40,values+36+i*12,16);
            if(tyre){
                memcpy(context+i*0x170+0xc0,values+76+i*10,32);
                unsigned char *wheel=body+0x1990+i*0x150;*(unsigned *)(context+i*0x170+0x178)=(unsigned)(uintptr_t)wheel;
                if(values[85+i*10]<=0.f){error=19;break;}
                memcpy(wheel+0x104,values+84+i*10,4);memcpy(wheel+0xe0,values+85+i*10,4);
            }
        }
        if(error)break;
        if(tyre)for(unsigned i=0;i<3;++i)*(float *)(context+0x6fc+i*4)=values[1+i]*3.f;
        __try {prepare(context,NULL);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        /* Original writes alternate inertia to its supplied body after copying.
           Velocity and pose must remain unchanged; returned coefficients use copy. */
        if(memcmp(state+8,context+(tyre?0x6fc:0x6f0),12)||memcmp(state+0x20,values+20,16)
           ||memcmp(state+0x30,values+24,16)||memcmp(state+0x40,values+4,64)){error=17;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",bits(*(float *)(context+i*0x170+0x70)));
        for(unsigned i=0;i<4;++i)for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",bits(*(float *)(context+i*0x170+0x88+j*4)));
        for(unsigned i=0;i<16;++i)fprintf(output,"\t%08x",bits(*(float *)(context+0x650+i*4)));
        if(tyre){
            for(unsigned a=0;a<2;++a)for(unsigned w=0;w<4;++w)fprintf(output,"\t%08x",*(unsigned *)(context+w*0x170+0x160+a*4));
            const unsigned offsets[3][3]={{0x88,0x98,0xa8},{0xe0,0xf0,0x100},{0x110,0x120,0x130}};
            for(unsigned s=0;s<3;++s)for(unsigned t=0;t<3;++t)if(s||t)for(unsigned w=0;w<4;++w)for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",*(unsigned *)(context+w*0x170+offsets[s][t]+j*4));
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    DWORD ignored;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,protection,&ignored))error=18;
    VirtualFree(image,0,MEM_RELEASE);
    printf("wheel_coupling_cases=%u error=%u\n",count,error);
    return error;
}
