/* Complete B11F40: supplied positive mass/diagonal inertia, owned empty world,
   no shapes. Allocation/resource/shape factory is outside this boundary. */
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
    unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 5;}memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
    unsigned error=0,protected=0;const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4],ignored;
    __declspec(align(16)) unsigned char request[0x16c],settings[0x160],owned[32+0x430+32],world[0x100];
    unsigned char *body=owned+32;FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
    int valid=fread(request,1,sizeof(request),input)==sizeof(request)&&fgetc(input)==EOF;fclose(input);input=NULL;
    if(!valid||get(request,0)!=0x31304350||get(request,4)>1||(get(request,8)!=0&&get(request,8)!=0xcd)){error=7;goto done;}
    memcpy(settings,request+12,sizeof(settings));
    if(settings[0x60]!=1||settings[0x61]||settings[0x62]||settings[0x63]||get(settings,0x64)||get(settings,0x134)||settings[0x130]>1||settings[0x131]>1||settings[0x132]>1||settings[0x133]>1){error=8;goto done;}
    for(unsigned i=0x68;i<0x90;i+=4)if(get(settings,i)){error=8;goto done;}
    for(unsigned i=0;i<0x160;i+=4)if(i!=0x60&&i!=0x64&&i!=0x130&&i!=0x134&&(!isfinite(number(settings,i))||((i<0xa0||i>0xb0)&&i!=0xec&&fabsf(number(settings,i))>1000000.f))){error=9;goto done;}
    if(number(settings,0)<=0){error=9;goto done;}
    for(unsigned r=0;r<4;++r)for(unsigned c=0;c<4;++c){float v=number(settings,0x10+r*16+c*4);if((r==c&&(v<=0||(r==3&&v!=1)))||(r!=c&&get(settings,0x10+r*16+c*4))){error=9;goto done;}}
    float cd=number(settings,0x24)*number(settings,0x38),determinant=cd*number(settings,0x10);
    if(!isnormal(determinant)||determinant<=0||!isfinite(1.f/number(settings,0))){error=9;goto done;}
    memset(owned,0xcd,sizeof(owned));memset(body,(int)get(request,8),0x430);memset(world,0,sizeof(world));put(world,0x60,get(request,4));put(settings,0x64,(unsigned)(uintptr_t)world);
    for(unsigned i=0;i<4;++i){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=10;goto done;}++protected;}
    __try{void *answer=((void *(__fastcall *)(void *,void *,void *))(image+0xb11f40-0x400000))(body,NULL,settings);if(answer!=body)error=11;}
    __except(fault(GetExceptionInformation())){error=12;}
    for(unsigned i=0;i<32;++i)if(owned[i]!=0xcd||owned[32+0x430+i]!=0xcd)error=13;
    for(unsigned i=0x90;i<0x2f0;i+=4)if(!isfinite(number(body,i)))error=19;
    for(unsigned i=0x3a4;i<=0x3b0;i+=4)if(!isfinite(number(body,i)))error=19;
    if(!isfinite(number(body,0x3f4))||!isfinite(number(body,0x3fc)))error=19;
    if(!error){
        const unsigned pointers[]={8,0xc,0x18,0x80,0x84,0x88,0x8c,0x330,0x334,0x338,0x33c,0x340};
        for(unsigned i=0;i<sizeof(pointers)/sizeof(*pointers);++i){unsigned p=get(body,pointers[i]);if(p<(unsigned)(uintptr_t)body||p>=(unsigned)(uintptr_t)(body+0x430)){error=14;break;}put(body,pointers[i],p-(unsigned)(uintptr_t)body+1);}
        if(get(body,0x32c)!=(unsigned)(uintptr_t)world||get(settings,0x64)!=(unsigned)(uintptr_t)world)error=14;
        put(body,0x32c,0x10000000);put(settings,0x64,0);
        for(unsigned i=0;i<0x100;i+=4){unsigned v=get(world,i);if(i==0x60){if(v!=get(request,4))error=14;}
            else if(v){if(v!=(unsigned)(uintptr_t)body)error=14;else put(world,i,1);}}
    }
done:
    for(unsigned i=0;i<protected;++i){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&ignored))error=15;}
    unsigned unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    if(!unchanged)error=16;
    if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(output==INVALID_HANDLE_VALUE)error=17;else{const void *parts[]={body,settings,world};const DWORD lengths[]={0x430,0x160,0x100};
            for(unsigned i=0;i<3;++i){DWORD written=0;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=18;}CloseHandle(output);}}
    if(input)fclose(input);free(snapshot);VirtualFree(image,0,MEM_RELEASE);
    printf("physical_bytes=1072 settings_bytes=352 world_bytes=256 code_unchanged=%u globals_restored=%u error=%u\n",unchanged,unchanged,error);return (int)error;
}
