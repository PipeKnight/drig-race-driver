/* Conditional original contact consumer: authored query-result fixture replaces
   the query call. Collision traversal is NOT validated. Material/radius helpers
   are original; all inputs are supplied, no live injection. */
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

typedef void (__fastcall *Initialize)(void *, void *,int);
static float fixture_normals[4][4],fixture_fractions[4],fixture_hits[4];
static unsigned char *fixture_material;
static unsigned query_count;
static int __cdecl observe(void *start,void *end,void *normal,void *distance,void *face){
    (void)start;(void)end;
    unsigned w=query_count++;
    if(w>=4)return 0;
    memcpy(normal,fixture_normals[w],16);memcpy(distance,fixture_fractions+w,4);
    *(unsigned *)(face)=(unsigned)(uintptr_t)fixture_material;
    return fixture_hits[w]!=0.f ? (int)(uintptr_t)fixture_material : 0;
}
static int instrument(unsigned char *image){
    unsigned char *call=image+0x9c18d7-0x400000;DWORD old,ignored;
    if(call[0]!=0xe8||*(uint32_t *)(call+1)!=0x96b7f0-0x9c18dc)return 0;
    if(!VirtualProtect(call,5,PAGE_READWRITE,&old))return 0;
    uint32_t relative=(uint32_t)(uintptr_t)&observe-(uint32_t)(uintptr_t)(call+5);
    memcpy(call+1,&relative,4);
    return VirtualProtect(call,5,old,&ignored)&&FlushInstructionCache(GetCurrentProcess(),call,5);
}
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    if(*(int *)(image+0xe85b1c-0x400000)!=0)return 18;
    if(!instrument(image))return 19;
    DWORD data_old;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&data_old))return 21;
    unsigned control;
    if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 23;
    printf("x87_control=%08x query_result_fixture=true\n",control);
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Initialize initialize=(Initialize)(image+0x9c1660-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[166];
        for(unsigned i=0;i<166;++i) {
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
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=body+0x1990+w*0x150;
            memcpy(context+0x20+w*0x170,values+16+w*4,16);
            memcpy(context+0x30+w*0x170,values+60+w*4,16);
            *(unsigned *)(context+0x178+w*0x170)=(unsigned)(uintptr_t)wheel;
            memcpy(body+0x1650+w*16,values+32+w*4,16);
            memcpy(wheel+0xe0,values+48+w,4);
            memcpy(wheel+0x48,values+52+w,4);
            memcpy(wheel+0x4c,values+56+w,4);
        }
        __declspec(align(16)) unsigned char material[0xb0]={0},table[0x20]={0};
        unsigned entry[3]={0};
        material[0x90]=(unsigned char)(values[124]!=0.f);
        memcpy(material+0x94,values+125,20);
        *(unsigned *)(table+0xc)=1;
        *(unsigned *)(table+0x14)=(unsigned)(uintptr_t)entry;
        *(unsigned *)(context+0x608)=(unsigned)(uintptr_t)table;
        memcpy(body+0x1648,values+130,4);memcpy(body+0x159c,values+131,4);memcpy(body+0x12ac,values+132,4);
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=body+0x1990+w*0x150;
            memcpy(wheel+0x110,values+76+w,4);
            memcpy(body+0x30c+w*40,values+80+w*10,40);
            memcpy(wheel+0x140,values+120+w,4);
            memcpy(context+0x7c+w*0x170,values+158+w,4);
            memcpy(context+0x80+w*0x170,values+162+w,4);
        }
        memcpy(fixture_fractions,values+133,16);memcpy(fixture_normals,values+137,64);memcpy(fixture_hits,values+153,16);
        fixture_material=material;
        query_count=0;
        __try {initialize(context,NULL,values[157]!=0.f);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        if(query_count!=4){error=20;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned w=0;w<4;++w){
            fprintf(output,"\t%08x\t%08x\t%08x",*(unsigned *)(context+0x68+w*0x170),*(unsigned *)(context+0x80+w*0x170),*(unsigned *)(context+0x7c+w*0x170));
            for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+0xd0+j*4));
            for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",*(unsigned *)(context+0x40+w*0x170+j*4));
            for(unsigned j=0;j<3;++j)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+0xc0+j*4)!=0);
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    printf("normal_lazy_flags=%08x\n",*(unsigned *)(image+0xe58bb0-0x400000));
    DWORD ignored;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,data_old,&ignored))error=22;
    VirtualFree(image,0,MEM_RELEASE);
    printf("contact_material_cases=%u error=%u\n",count,error);
    return error;
}
