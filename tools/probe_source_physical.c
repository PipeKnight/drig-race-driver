/* Whole 9B8D00 -> descriptor -> A742E0 -> original pool/B11F40/wrapper.
   Own empty world, one-cell allocator, actual host critical section imports.
   No shapes or resource factory; never launch or modify the Steam reference. */
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
#define CAR 0x370a0
#define REQUEST (12+0x65c+CAR)
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static float number(const void *p,unsigned o){float v;memcpy(&v,(const char *)p+o,4);return v;}
static uintptr_t reference_base;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference_base+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;reference_base=(uintptr_t)image;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage),*request=malloc(REQUEST);
    if(!snapshot||!request){free(snapshot);free(request);VirtualFree(image,0,MEM_RELEASE);return 5;}memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
    unsigned error=0,protected=0,cs_ready=0;const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xc89000,0xe8f000};DWORD protections[6],ignored;
    __declspec(align(16)) unsigned char owned[32+16+0x440+32],world[0x100],pool[0x40],manager[0x30],points[64];
    unsigned char *cell=owned+32,*body=cell+16,*wrapper=body+0x430,*props=request+12,*car=props+0x65c;
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
    int valid=fread(request,1,REQUEST,input)==REQUEST&&fgetc(input)==EOF;fclose(input);input=NULL;
    if(!valid||get(request,0)!=0x31304650||get(request,4)>1||(get(request,8)!=0&&get(request,8)!=0xcd)||get(car,0xdb0)||get(car,0xdd8)){error=7;goto done;}
    for(unsigned i=0x130;i<0x140;i+=4)if(!isfinite(number(props,i))||number(props,i)<=0||number(props,i)>1000000.f){error=8;goto done;}
    float cd=number(props,0x138)*number(props,0x13c),determinant=cd*number(props,0x134);
    if(!isnormal(determinant)||determinant<=0||!isfinite(1.f/number(props,0x130))){error=8;goto done;}
    memset(owned,0xcd,sizeof(owned));memset(body,(int)get(request,8),0x440);put(cell,0,0);
    memset(world,0,sizeof(world));put(world,0x60,get(request,4));memset(pool,0,sizeof(pool));memset(manager,0,sizeof(manager));memset(points,0,sizeof(points));
    put(pool,4,(unsigned)(uintptr_t)cell);put(pool,0xc,1);put(pool,0x14,16);
    InitializeCriticalSection((CRITICAL_SECTION *)(pool+0x1c));cs_ready=1;
    put(manager,4,(unsigned)(uintptr_t)world);put(manager,0x24,(unsigned)(uintptr_t)pool);put(car,0xdd8,(unsigned)(uintptr_t)manager);
    for(unsigned i=0;i<6;++i){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=9;goto done;}++protected;}
    put(image,0xc89230-0x400000,(unsigned)(uintptr_t)EnterCriticalSection);put(image,0xc8922c-0x400000,(unsigned)(uintptr_t)LeaveCriticalSection);put(image,0xe8f068-0x400000,0);
    __try{((void (__fastcall *)(void *,void *,void *,void *))(image+0x9b8d00-0x400000))(car,NULL,props,points);}
    __except(fault(GetExceptionInformation())){error=10;}
    for(unsigned i=0;i<32;++i)if(owned[i]!=0xcd||owned[32+16+0x440+i]!=0xcd)error=11;
    if(!error){
        if(get(car,0xdb0)!=(unsigned)(uintptr_t)wrapper||get(car,0xdd8)!=(unsigned)(uintptr_t)manager||get(body,0x424)!=(unsigned)(uintptr_t)wrapper||get(wrapper,0)!=(unsigned)(uintptr_t)(image+0xd314e4-0x400000)||get(wrapper,4)||get(wrapper,8)!=(unsigned)(uintptr_t)body||get(pool,4)||get(pool,0x10)!=1||get(cell,0)!=(unsigned)(uintptr_t)pool||number(image,0xe8f068-0x400000)!=0.f-number(image,0xcb3ba8-0x400000)){error=12;goto done;}
        const unsigned pointers[]={8,0xc,0x18,0x80,0x84,0x88,0x8c,0x330,0x334,0x338,0x33c,0x340};
        for(unsigned i=0;i<sizeof(pointers)/sizeof(*pointers);++i){unsigned p=get(body,pointers[i]);if(p<(unsigned)(uintptr_t)body||p>=(unsigned)(uintptr_t)(body+0x430)){error=12;break;}put(body,pointers[i],p-(unsigned)(uintptr_t)body+1);}
        if(get(body,0x32c)!=(unsigned)(uintptr_t)world)error=12;
        put(body,0x32c,0x10000000);put(body,0x424,0x20000000);put(car,0xdb0,1);put(car,0xdd8,0);put(wrapper,0,0xd314e4);put(wrapper,8,1);put(cell,0,0x30000000);
        for(unsigned i=0;i<0x100;i+=4){unsigned value=get(world,i);if(i==0x60){if(value!=get(request,4))error=12;}
            else if(value){if(value!=(unsigned)(uintptr_t)body)error=12;else put(world,i,1);}}
        for(unsigned i=0x90;i<0x2f0;i+=4)if(!isfinite(number(body,i)))error=13;
    }
done:
    if(cs_ready)DeleteCriticalSection((CRITICAL_SECTION *)(pool+0x1c));
    for(unsigned i=0;i<protected;++i){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&ignored))error=14;}
    unsigned unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    if(!unchanged)error=15;
    if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(output==INVALID_HANDLE_VALUE)error=16;else{const void *parts[]={car,body,wrapper,world,pool,cell};const DWORD lengths[]={CAR,0x430,16,0x100,0x1c,16};
            for(unsigned i=0;i<6;++i){DWORD written=0;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=17;}CloseHandle(output);}}
    if(input)fclose(input);free(snapshot);free(request);VirtualFree(image,0,MEM_RELEASE);
    printf("source_factory=1 host_lock_imports=2 code_unchanged=%u globals_restored=%u error=%u\n",unchanged,unchanged,error);return (int)error;
}
