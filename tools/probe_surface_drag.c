/* Whole four-wheel 9dfc50 with original cross/impulse helpers. No code patches. */
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
typedef void (__cdecl *Kernel)(void *,float);
typedef unsigned *(__fastcall *Lookup)(void *,void *,unsigned);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4&&argc!=5)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    DWORD protection,ignored;if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&protection))return 5;
    Kernel kernel=(Kernel)(image+0x9dfc50-0x400000);
    Lookup lookup=(Lookup)(image+0xb3b0c0-0x400000);unsigned source_rows[4096*5]={0},source_count=0;
    if(argc==5){
        FILE *source=NULL;char tag[4];if(_wfopen_s(&source,argv[4],L"rb")||!source)return 16;
        if(fread(tag,1,4,source)!=4||memcmp(tag,"DRG1",4)||fread(&source_count,4,1,source)!=1||!source_count||source_count>4096)return 17;
        if(fread(source_rows,20,source_count,source)!=source_count||fgetc(source)!=EOF||ferror(source)||fclose(source))return 18;
        for(unsigned i=0;i<source_count;++i){unsigned *q=source_rows+i*5;if(q[1]>1)return 19;float coeffs[3];memcpy(coeffs,q+2,12);for(unsigned j=0;j<3;++j)if(!isfinite(coeffs[j]))return 19;if(q[1]&&coeffs[0]==coeffs[1])return 19;}
    }
    unsigned key_table[5]={0,(unsigned)(uintptr_t)source_rows,source_count,20,1};
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 6;
    char line[2048],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++cases>4096||!strchr(line,'\n')){error=7;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=8;break;}memcpy(name,start,length);name[length]=0;
        float v[82];
        for(unsigned i=0;i<(argc==5?82u:78u);++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long bitsword=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=9;break;}unsigned w=(unsigned)bitsword;memcpy(v+i,&w,4);cursor=stop;if(i<78&&!isfinite(v[i])){error=9;break;}}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||v[0]<0.f||v[0]>.1f||v[2]<=0.f||v[3]<=0.f||v[4]<=0.f||v[5]<=0.f){error=10;break;}
        __declspec(align(16)) unsigned char body[0x2300]={0},view[0x300]={0},materials[4][0xb0]={0};
        *(unsigned *)body=(unsigned)(uintptr_t)view;*(unsigned *)(body+0x1620)=4;*(float *)(body+0x1648)=v[1];*(float *)(body+0x15a0)=v[2];memcpy(body+0x15a8,v+3,12);memcpy(body+0x15e0,v+6,64);memcpy(body+0x15c0,v+22,32);
        unsigned selected[4]={UINT32_MAX,UINT32_MAX,UINT32_MAX,UINT32_MAX};
        for(unsigned i=0;i<4;++i){float *q=v+30+i*12;unsigned char *wheel=body+0x1990+i*0x150;
            if(q[5]<=0.f||(q[7]!=0.f&&q[7]!=1.f)||(q[8]!=0.f&&q[8]!=1.f)||(argc!=5&&q[7]!=0.f&&q[9]==q[10])){error=11;break;}
            memcpy(view+0x250+i*16,q,16);*(float *)(wheel+0x104)=q[4];*(float *)(wheel+0xe0)=q[5];*(float *)(body+0x22b4+i*4)=q[6];
            if(q[7]!=0.f)*(unsigned *)(wheel+0xc0)=(unsigned)(uintptr_t)materials[i];materials[i][0x73]=(unsigned char)q[8];memcpy(materials[i]+0x14,q+9,12);
            if(argc==5&&q[7]!=0.f){
                unsigned *found=lookup(key_table,NULL,bits(v[78+i]));
                if(!found||found<source_rows||found>=source_rows+source_count*5||(found-source_rows)%5||!found[1]){error=20;break;}
                selected[i]=(unsigned)(found-source_rows)/5;materials[i][0x73]=0;memcpy(materials[i]+0x14,found+2,12);
            }
        }
        if(error)break;
        __try{kernel(body,v[0]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=12;break;}
        fprintf(output,"P\t%s",name);for(unsigned i=0;i<8;++i){float f=*(float *)(body+0x15c0+i*4);if(!isfinite(f))error=13;fprintf(output,"\t%08x",bits(f));}
        for(unsigned i=0;i<4;++i){float f=*(float *)(body+0x1a94+i*0x150);if(!isfinite(f))error=13;fprintf(output,"\t%08x",bits(f));}
        if(argc==5)for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",selected[i]);fputc('\n',output);if(error)break;
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=14;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,protection,&ignored))error=15;
    VirtualFree(image,0,MEM_RELEASE);printf("surface_drag_cases=%u error=%u\n",cases,error);return error;
}
