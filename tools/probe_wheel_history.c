/* Whole original 9ba490; supplied body/wheel/material/vectors and angle.
   Original grip/trigonometric calls, table inactive; no code/data patches. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef double (__fastcall *Kernel)(void *,void *,void *,void *,unsigned,float,float,float,void *,void *,float);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4&&argc!=5)return 2;
    int angled=argc==5;if(angled&&wcscmp(argv[4],L"angle"))return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    if(image[0xe8efbc-0x400000])return 5;
    Kernel kernel=(Kernel)(image+0x9ba490-0x400000);
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 6;
    char line[2048],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++cases>4096||!strchr(line,'\n')){error=7;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=8;break;}memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float v[61];
        for(unsigned i=0;i<(angled?61u:56u);++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long bitsword=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=9;break;}unsigned w=(unsigned)bitsword;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=9;break;}}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor||v[0]<0.f||v[0]>.1f||v[3]<=0.f||v[4]<=0.f||v[24]<=0.f||v[16]<0.f||v[9]<0.f||v[9]>=1.f){error=10;break;}
        const unsigned flags[]={10,11,12,13,18,30,31};for(unsigned i=0;i<7;++i)if(v[flags[i]]!=0.f&&v[flags[i]]!=1.f)error=10;if(v[35]!=0.f&&v[35]!=1.f&&v[35]!=2.f)error=10;if(error)break;
        __declspec(align(16)) unsigned char body[0x2300]={0},wheel[0x150]={0},material[0xb0]={0},view[0x300]={0};unsigned stamp=0;
        *(unsigned *)body=(unsigned)(uintptr_t)view;
        const unsigned bo[]={0x1648,0x15a0,0x1f3c,0x1ffc,0x1efc,0xe20,0x187c,0x137c};for(unsigned i=0;i<8;++i)*(float *)(body+bo[i])=v[2+i];
        body[0xb11]=(unsigned char)v[10];body[0xe38]=(unsigned char)v[11];memcpy(body+0x15e0,v+52,16);*(float *)(body+0x15f4)=1.f;
        if(angled){*(float *)(wheel+0xec)=v[56];memcpy(body+0x15f0,v+57,16);}
        material[0x70]=(unsigned char)v[12];material[0x72]=(unsigned char)v[13];*(float *)(material+0x6c)=v[14];memcpy(material,v+15,12);material[0xac]=1;
        if(v[18]!=0.f)*(unsigned *)(wheel+0xc0)=(unsigned)(uintptr_t)material;
        const unsigned wo[]={0xb8,0xbc,0x6c,0x80,0x104,0xe0,0x138,0xe4,0xb0,0xb4,0x10c};for(unsigned i=0;i<11;++i)*(float *)(wheel+wo[i])=v[19+i];
        wheel[0xfc]=(unsigned char)v[30];wheel[0xfd]=(unsigned char)v[31];memcpy(wheel+0x11c,v+32,12);
        *(unsigned *)(wheel+0x128)=v[35]==1.f?(unsigned)(uintptr_t)material:v[35]==2.f?(unsigned)(uintptr_t)&stamp:0;
        memcpy(view+0x190,v+36,16);memcpy(view+0x210,v+40,16);memcpy(view+0x1d0,v+44,16);memcpy(wheel+0xd0,v+48,16);
        float result=0;
        __try{result=(float)kernel(body,NULL,view+0x250,wheel,0,0.f,0.f,v[0],NULL,NULL,v[1]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=11;break;}
        unsigned actual_stamp=*(unsigned *)(wheel+0x128),token=actual_stamp==(unsigned)(uintptr_t)material?1:actual_stamp==(unsigned)(uintptr_t)&stamp?2:0;if(actual_stamp&&!token){error=12;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\t%08x\t%08x",name,*(unsigned *)(wheel+0xb8),*(unsigned *)(wheel+0xbc),*(unsigned *)(wheel+0x6c),(unsigned)wheel[0xfd]);
        for(unsigned i=0;i<3;++i)fprintf(output,"\t%08x",*(unsigned *)(wheel+0x11c+i*4));fprintf(output,"\t%08x\t%08x\n",token,bits(result));
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=13;VirtualFree(image,0,MEM_RELEASE);printf("wheel_history_cases=%u error=%u\n",cases,error);return error;
}
