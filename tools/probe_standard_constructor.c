/* Whole A0C9F0 on owned Standard properties, geometry and zero cold storage.
   Caller/asset selection, original CSV parser and complete outer factory excluded.
   No original code replacements, entry point, imports or TLS initialization. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <float.h>
#include <math.h>
#include "isolated_reference.h"
typedef void *(__cdecl *Construct)(void *,const void *,float *,const float *,const float *);
static uintptr_t reference_base;
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static LONG fault(EXCEPTION_POINTERS *e){
    fprintf(stderr,"fault=%08lx original_eip=%08lx access=%lu original_address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference_base+0x400000),e->ExceptionRecord->ExceptionInformation[0],(DWORD)(e->ExceptionRecord->ExceptionInformation[1]-reference_base+0x400000));
    return EXCEPTION_EXECUTE_HANDLER;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    reference_base=(uintptr_t)image;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    unsigned char *code_snapshot=malloc(pe->OptionalHeader.SizeOfImage);
    if(!code_snapshot){VirtualFree(image,0,MEM_RELEASE);return 17;}
    memcpy(code_snapshot,image,pe->OptionalHeader.SizeOfImage);
    unsigned error=0,protected=0,globals_restored=1;FILE *input=NULL;
    __declspec(align(16)) unsigned char request[1732];
    __declspec(align(16)) unsigned char props[0x65c];
    __declspec(align(16)) float points[16],radii[4],widths[4];
    __declspec(align(16)) unsigned char owned[32+0x21000+64];
    unsigned char *body=owned+32;memset(owned,0xcd,sizeof(owned));memset(body,0,0x21000);
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xd16000,0xe85000,0xe8e000,0xe4f000};
    unsigned char saved[sizeof(pages)/sizeof(*pages)][4096];DWORD protections[sizeof(pages)/sizeof(*pages)];
    if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
    int valid=fread(request,1,sizeof(request),input)==sizeof(request)&&fgetc(input)==EOF;fclose(input);input=NULL;
    unsigned source_prelude=get(request,0)==0x33304353;
    if(!valid||(get(request,0)!=0x32304353&&!source_prelude)){error=6;goto done;}
    memcpy(props,request+8,sizeof(props));memcpy(points,request+8+sizeof(props),sizeof(points));
    memcpy(radii,request+8+sizeof(props)+sizeof(points),sizeof(radii));
    memcpy(widths,request+8+sizeof(props)+sizeof(points)+sizeof(radii),sizeof(widths));
    if(get(props,0x140)!=1||props[0x1d4]!=1||props[0x308]!=1||get(props,0x304)<1||get(props,0x304)>3){error=7;goto done;}
    float mass,rpm;memcpy(&mass,props+0x130,4);memcpy(&rpm,props+0x1e8,4);
    if(!isfinite(mass)||mass<=0.f||mass>1000000.f||!isfinite(rpm)||rpm<0.f||rpm>20000.f){error=8;goto done;}
    for(unsigned i=0;i<32;++i){float value;memcpy(&value,props+0x1f4+i*4,4);if(!isfinite(value)){error=8;goto done;}}
    for(unsigned i=0;i<10;++i){float value;memcpy(&value,props+0x318+i*4,4);if(!isfinite(value)){error=8;goto done;}}
    for(unsigned i=0;i<16;++i)if(!isfinite(points[i])||fabsf(points[i])>100.f){error=8;goto done;}
    for(unsigned i=0;i<4;++i)if(!isfinite(radii[i])||radii[i]<=0.f||radii[i]>10.f||!isfinite(widths[i])||widths[i]<=0.f||widths[i]>10.f){error=8;goto done;}
    if(source_prelude){
        memcpy(body+0xe78,props,0x65c);float one=1.f;unsigned value=10;
        memcpy(body+0x1560,&one,4);memcpy(body+0x1564,props+0x3e0,4);memcpy(body+0xe48,&value,4);
        value=4;memcpy(body+0x9a8,&value,4);value=2;memcpy(body+0xba0,&value,4);
        body[0xb13]=body[0xb14]=body[0x1ee0]=body[0x21c8]=1;
        for(unsigned i=0;i<4;++i){memcpy(body+0x10+i*20,&one,4);memcpy(body+0x15e0+i*20,&one,4);}
        value=(unsigned)(uintptr_t)body;memcpy(body+0x9a4,&value,4);
    }
    for(unsigned i=0;i<sizeof(pages)/sizeof(*pages);++i){
        unsigned char *p=image+pages[i]-0x400000;memcpy(saved[i],p,4096);
        if(!VirtualProtect(p,4096,PAGE_READWRITE,protections+i)){error=9;goto done;}++protected;
    }
    float gravity=-9.8100004196167f;memcpy(image+0xe8efd4-0x400000,&gravity,4);
    unsigned random_state=get(request,4);memcpy(image+0xe4fcc0-0x400000,&random_state,4);
    Construct construct=(Construct)(image+0xa0c9f0-0x400000);void *answer=NULL;
    __try{answer=construct(body,props,points,radii,widths);}
    __except(fault(GetExceptionInformation())){error=10;}
    memcpy(&random_state,image+0xe4fcc0-0x400000,4);
    if(!error)printf("random_state=%08x\n",random_state);
    if(!error&&answer!=body)error=11;
    for(unsigned i=0;i<32;++i)if(owned[i]!=0xcd)error=12;
    for(unsigned i=32+0x21000;i<sizeof(owned);++i)if(owned[i]!=0xcd)error=12;
    if(!error){
        /* Canonicalize only constructor-owned addresses inside the body. Keep
           every numeric/state byte; pointer identity remains an explicit token. */
        const unsigned pointers[]={0x18d8,0x18dc,0x18e0,0x18e4,0x21cc,0x21d0,0xc7f8,
            0x19c4,0x1b14,0x1c64,0x1db4};
        for(unsigned i=0;i<sizeof(pointers)/sizeof(*pointers);++i){unsigned value=get(body,pointers[i]);
            if(value){if(value<(unsigned)(uintptr_t)body||value>=(unsigned)(uintptr_t)(body+0x21000)){error=13;break;}
                value-=(unsigned)(uintptr_t)body;memcpy(body+pointers[i],&value,4);}}
        unsigned callback=get(body,0xc784);
        if(source_prelude){unsigned self=get(body,0x9a4);if(self!=(unsigned)(uintptr_t)body)error=13;else memset(body+0x9a4,0,4);}
        if(callback!=(unsigned)(uintptr_t)(image+0x9a0c90-0x400000))error=13;
        else{callback=0x9a0c90;memcpy(body+0xc784,&callback,4);}
        /* 990610 writes 22 byte flags at settings48..5D; settings5E/5F
           are untouched stack padding copied into body202E/202F twice.
           These two undefined padding bytes are explicitly excluded. */
        body[0x202e]=0;body[0x202f]=0;
    }
done:
    for(unsigned i=0;i<protected;++i){DWORD ignored;unsigned char *p=image+pages[i]-0x400000;memcpy(p,saved[i],4096);
        if(!VirtualProtect(p,4096,protections[i],&ignored)||memcmp(p,saved[i],4096)){error=16;globals_restored=0;}}
    unsigned code_unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,code_snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))code_unchanged=0;
    if(!code_unchanged)error=18;
    if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(output==INVALID_HANDLE_VALUE)error=14;else{DWORD written=0;if(!WriteFile(output,body,0x21000,&written,NULL)||written!=0x21000)error=15;CloseHandle(output);}}
    if(input)fclose(input);free(code_snapshot);VirtualFree(image,0,MEM_RELEASE);
    printf("constructor_bytes=135168 excluded_stack_padding_bytes=2 code_unchanged=%u code_replacements=0 globals_restored=%u error=%u\n",code_unchanged,globals_restored,error);return (int)error;
}
