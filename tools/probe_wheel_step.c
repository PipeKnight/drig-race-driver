/* Whole original substep, four active wheels, extra tyre axes inactive.
   Supplied coupling/rates/contact/travel bounds; original stop helper unstubbed. */
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
#include "isolated_reference.h"
typedef void (__fastcall *Kernel)(void *,void *,float,float);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    FILE *input=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);fclose(input);return 7;}
    Kernel kernel=(Kernel)(image+0x9c1f60-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[139];
        for(unsigned i=0;i<139;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||values[0]<=0.f||values[0]>.25f||values[2]<=0.f){error=12;break;}
        __declspec(align(16)) unsigned char body[0x2600]={0},context[0x750]={0};
        *(uint32_t *)(context+0x5d4)=(uint32_t)(uintptr_t)body;*(uint32_t *)(context+0x5d8)=4;
        *(float *)context=values[2];memcpy(context+0x650,values+19,64);
        for(unsigned i=0;i<4;++i) {
            float *v=values+35+i*26;unsigned char *w=body+0x1990+i*0x150,*c=context+i*0x170;
            if(v[4]<=0.f||v[6]<0.f||fabsf(v[0])>=1.f||fabsf(v[1])>=20.f||values[0]>.01f||(v[23]!=0.f&&v[23]!=1.f)){error=16;break;}
            *(uint32_t *)(c+0x178)=(uint32_t)(uintptr_t)w;
            if(v[24]>v[25]||fabsf(v[24])>=99.f||fabsf(v[25])>=99.f){error=18;break;}
            *(float *)(w+0x50)=v[25];*(float *)(w+0x48)=v[24];
            *(float *)(c+0x64)=v[0];*(float *)(c+0x74)=v[1];*(float *)(c+0x60)=v[2];
            const unsigned offsets[8]={0x54,0x4c,0x60,0x64,0x68,0x114,0x58,0x70};
            for(unsigned j=0;j<8;++j)*(float *)(w+offsets[j])=v[3+j];
            memcpy(c+0x30,v+11,16);*(float *)(c+0x70)=v[15];*(float *)(context+0x640+i*4)=v[16];
            *(float *)(c+0x68)=v[17];*(float *)(c+0x78)=v[18];
            *(float *)(c+0x168)=v[19];*(float *)(c+0x16c)=v[20];*(float *)(c+0x174)=v[21];*(float *)(c+0x170)=v[22];
            *(unsigned char *)(c+0x84)=(unsigned char)v[23];memcpy(c+0x88,values+3+i*4,16);
        }
        if(error)break;
        __try {kernel(context,NULL,values[0],values[1]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        const unsigned fields[9]={0x64,0x74,0x70,0x68,0x78,0x168,0x16c,0x174,0x170};
        for(unsigned i=0;i<4;++i) {
            unsigned char *c=context+i*0x170;
            if(fabsf(*(float *)(c+0x64))>=99.f){error=17;break;}
            for(unsigned j=0;j<9;++j)fprintf(output,"\t%08x",bits(*(float *)(c+fields[j])));
            fprintf(output,"\t%08x",bits(*(unsigned char *)(c+0x84)?1.f:0.f));
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;fclose(input);if(fclose(output))error=15;
    VirtualFree(image,0,MEM_RELEASE);printf("wheel_step_cases=%u error=%u\n",count,error);return error;
}
