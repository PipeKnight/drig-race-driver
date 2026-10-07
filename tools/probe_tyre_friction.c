/* Whole original active substep; neutral differential/brake/cross-wheel responses. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef void (__fastcall *Kernel)(void *,void *,float,float);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);int coupled=argc==5&&!wcscmp(argv[4],L"coupled");if(argc!=4&&!coupled)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 4;
    Kernel kernel=(Kernel)(image+0x9c1f60-0x400000);char line[8192],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++cases>4096||!strchr(line,'\n')){error=5;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=6;break;}memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[207];
        for(unsigned i=0;i<(coupled?207u:43u);++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long bitsword=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=7;break;}unsigned word=(unsigned)bitsword;memcpy(values+i,&word,4);cursor=stop;if(!isfinite(values[i])){error=8;break;}}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor||values[0]<=0.f||values[0]>.01f){error=9;break;}
        __declspec(align(16)) unsigned char body[0x2600]={0},context[0x750]={0};
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;*(unsigned *)(context+0x5d8)=4;context[0x70c]=1;*(float *)context=1.f;*(float *)(context+0x708)=1.f;
        *(float *)(body+0x1324)=1.f;*(float *)(body+0x1328)=2.f;*(float *)(body+0x1330)=1.f;*(float *)(body+0x1334)=2.f;*(float *)(body+0x133c)=1.f;*(float *)(body+0x1340)=2.f;*(float *)(body+0x11ec)=values[2];
        for(unsigned w=0;w<4;++w){
            float *v=values+3+w*10;unsigned char *wheel=body+0x1990+w*0x150,*c=context+w*0x170;
            if(v[0]<0.f||v[0]>5.f||v[1]<=0.f||v[3]<=0.f||v[5]<=0.f){error=10;break;}
            *(unsigned *)(c+0x178)=(unsigned)(uintptr_t)wheel;*(float *)(c+0x34)=1.f;*(float *)(c+0x78)=v[0];
            *(float *)(wheel+0x48)=-98.f;*(float *)(wheel+0x50)=98.f;*(float *)(wheel+0x4c)=1.f;*(float *)(wheel+0x64)=1.f;*(float *)(wheel+0x114)=1.f;
            *(float *)(wheel+0xe0)=v[1];*(float *)(wheel+0x104)=v[2];*(float *)(c+0x158)=v[3];*(float *)(c+0x154)=v[4];*(float *)(c+0x15c)=v[5];*(float *)(c+0x160)=v[6];*(float *)(c+0x150)=v[7];*(float *)(c+0x164)=v[8];*(float *)(context+0x710+w*4)=v[9];
            if(coupled){*(float *)(c+0x60)=1.f;*(float *)(wheel+0x54)=values[43+w];}
        }
        if(error)break;
        if(coupled){
            const unsigned offsets[3][3]={{0x88,0x98,0xa8},{0xe0,0xf0,0x100},{0x110,0x120,0x130}};
            for(unsigned s=0;s<3;++s)for(unsigned t=0;t<3;++t)for(unsigned w=0;w<4;++w)memcpy(context+w*0x170+offsets[s][t],values+47+(s*3+t)*16+w*4,16);
            memcpy(context+0x650,values+191,64);
        }
        __try{kernel(context,NULL,values[0],values[1]);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=11;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned w=0;w<4;++w){unsigned char *c=context+w*0x170,*wheel=body+0x1990+w*0x150;const unsigned offsets[9]={0x174,0x68,0x74,0x64,0x150,0x140,0x144,0x148,0x14c};for(unsigned j=0;j<9;++j)fprintf(output,"\t%08x",*(unsigned *)(c+offsets[j]));fprintf(output,"\t%08x\t%08x",*(unsigned *)(wheel+0x104),*(unsigned *)(c+0x164));if(coupled)fprintf(output,"\t%08x\t%08x",*(unsigned *)(c+0x70),*(unsigned *)(c+0x160));}fputc('\n',output);
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;VirtualFree(image,0,MEM_RELEASE);printf("tyre_friction_cases=%u error=%u\n",cases,error);return error;
}
