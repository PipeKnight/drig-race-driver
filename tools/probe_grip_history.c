/* Whole original 98db40 with supplied per-wheel/body inputs; no patches/stubs. */
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
typedef double (__cdecl *Kernel)(void *,unsigned);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    Kernel kernel=(Kernel)(image+0x98db40-0x400000);
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
    char line[1024],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++cases>4096||!strchr(line,'\n')){error=6;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=7;break;}memcpy(name,start,length);name[length]=0;
        float v[12];
        for(unsigned i=0;i<12;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long bitsword=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)bitsword;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||v[0]<0.f||v[0]>3.f||v[0]!=(float)(unsigned)v[0]||(v[1]!=0.f&&v[1]!=1.f)||(v[2]!=0.f&&v[2]!=1.f)){error=9;break;}
        unsigned ordinal=(unsigned)v[0];__declspec(align(16)) unsigned char body[0x2300]={0};
        unsigned char *wheel=body+0x1990+ordinal*0x150;
        body[0xb12]=(unsigned char)v[1];body[0x227c+ordinal]=(unsigned char)v[2];
        memcpy(wheel+0x11c,v+3,12);
        const unsigned offsets[]={0x1ffc,0x1f24,0x1fa4,0x1fa8};for(unsigned i=0;i<4;++i)*(float *)(body+offsets[i])=v[6+i];*(float *)(wheel+8)=v[10];
        float factor=0,result=0;
        __try{double returned=kernel(body,ordinal);factor=(float)returned;result=(float)(returned*(double)v[11]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
        if(!isfinite(factor)||!isfinite(result)){error=11;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\n",name,bits(factor),bits(result));
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;VirtualFree(image,0,MEM_RELEASE);printf("grip_history_cases=%u error=%u\n",cases,error);return error;
}
