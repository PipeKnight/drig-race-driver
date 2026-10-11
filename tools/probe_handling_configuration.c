/* Whole original 9C5580 on an owned binary-XML document view. Original text
   tokenization/resource acquisition excluded. Optional host numeric CRT adapter. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <float.h>
#include "isolated_reference.h"
#include "surface_xml_probe.h"
typedef unsigned char (__fastcall *Load)(void *,void *,void *);
typedef void (__fastcall *Reset)(void *,void *);
static double __cdecl number_adapter(const char *s){return strtod(s,NULL);}
static uintptr_t reference_base;
static LONG fault(EXCEPTION_POINTERS *e){
    fprintf(stderr,"fault=%08lx original_eip=%08lx access=%lu address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference_base+0x400000),e->ExceptionRecord->ExceptionInformation[0],(DWORD)e->ExceptionRecord->ExceptionInformation[1]);
    return EXCEPTION_EXECUTE_HANDLER;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if((argc!=5&&argc!=8)||(wcscmp(argv[4],L"original")&&wcscmp(argv[4],L"adapted")))return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;reference_base=(uintptr_t)image;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);
    if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 5;}memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
    unsigned error=0,adapted=!wcscmp(argv[4],L"adapted"),patched=0;DWORD patch_protection=0,ignored;
    unsigned doc[9]={0};unsigned char *xml=read_xml(argv[2],doc);
    __declspec(align(16)) unsigned char storage[32+0x168+32];memset(storage,0xcd,sizeof(storage));memset(storage+32,0,0x168);
    __declspec(align(16)) unsigned char car[32+0x21000+32],wrapper[0x30]={0};
    unsigned char rng_saved[4096];DWORD rng_protection=0;unsigned rng_writable=0,random_state=0;
    unsigned char *destination=storage+32,*result_bytes=destination;unsigned result_size=0x168;
    if(!xml||!doc[2]||strcmp(text(doc,word((void *)(uintptr_t)doc[3])),"VEHICLEHANDLINGDAMAGE")){error=6;goto done;}
    /* Original binary-document vtable; original root/child/accessor dispatch.
       The bounded host view supplies already decoded tables, without allocation. */
    doc[0]=(unsigned)(uintptr_t)(image+0xd3d508-0x400000);unsigned *document=doc;
    if(argc==8){
        if(wcscmp(argv[7],L"0")&&wcscmp(argv[7],L"1")){error=15;goto done;}
        wchar_t *end;unsigned long seed=wcstoul(argv[6],&end,16);
        if(!*argv[6]||*end||wcslen(argv[6])>8){error=15;goto done;}random_state=seed;
        FILE *input=NULL;if(_wfopen_s(&input,argv[5],L"rb")||!input){error=15;goto done;}
        memset(car,0xcd,sizeof(car));int valid=fread(car+32,1,0x21000,input)==0x21000&&fgetc(input)==EOF;fclose(input);
        if(!valid||word(car+32+0xcdf0+0x210)||word(car+32+0xcdf0+0x214)||word(car+32+0x1620)!=4){error=15;goto done;}
        destination=car+32+0xcdf0+0x218;result_bytes=car+32;result_size=0x21000;
        memcpy(rng_saved,image+0xe4f000-0x400000,4096);
        if(!VirtualProtect(image+0xe4f000-0x400000,4096,PAGE_READWRITE,&rng_protection)){error=16;goto done;}rng_writable=1;
        memcpy(image+0xe4fcc0-0x400000,&random_state,4);
        if(!wcscmp(argv[7],L"1")){unsigned p=(unsigned)(uintptr_t)wrapper;memcpy(car+32+0xcdf0+0x214,&p,4);p=(unsigned)(uintptr_t)(car+32);memcpy(wrapper+0x24,&p,4);}
    }
    if(adapted){unsigned char *entry=image+0xbf3b60-0x400000;
        if(!VirtualProtect(entry,5,PAGE_READWRITE,&patch_protection)){error=7;goto done;}
        entry[0]=0xe9;unsigned relative=(unsigned)(uintptr_t)&number_adapter-(unsigned)(uintptr_t)(entry+5);memcpy(entry+1,&relative,4);patched=1;
        if(!VirtualProtect(entry,5,patch_protection,&ignored)||!FlushInstructionCache(GetCurrentProcess(),entry,5)){error=7;goto done;}}
    Load load=(Load)(image+0x9c5580-0x400000);unsigned char result=0;
    __try{result=load(destination,NULL,&document);
        if(argc==8){Reset reset=(Reset)(image+0x9e6070-0x400000);reset(car+32+0xcdf0,NULL);}}
    __except(fault(GetExceptionInformation())){error=8;}
    if(!error&&result!=1)error=9;
    for(unsigned i=0;i<32;++i)if(storage[i]!=0xcd||storage[32+0x168+i]!=0xcd)error=10;
    if(argc==8&&!error){
        for(unsigned i=0;i<32;++i)if(car[i]!=0xcd||car[32+0x21000+i]!=0xcd)error=10;
        memset(car+32+0xcdf0+0x214,0,4);memcpy(&random_state,image+0xe4fcc0-0x400000,4);
        printf("random_state=%08x\n",random_state);
    }
done:
    if(rng_writable){memcpy(image+0xe4f000-0x400000,rng_saved,4096);if(!VirtualProtect(image+0xe4f000-0x400000,4096,rng_protection,&ignored))error=16;}
    if(patched){unsigned char *entry=image+0xbf3b60-0x400000;DWORD old;
        if(!VirtualProtect(entry,5,PAGE_READWRITE,&old))error=11;
        else{memcpy(entry,snapshot+0xbf3b60-0x400000,5);if(!VirtualProtect(entry,5,patch_protection,&ignored)||!FlushInstructionCache(GetCurrentProcess(),entry,5))error=11;}}
    unsigned restored=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))restored=0;
    if(!restored)error=12;
    if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(output==INVALID_HANDLE_VALUE)error=13;
        else{DWORD written=0;if(!WriteFile(output,result_bytes,result_size,&written,NULL)||written!=result_size)error=14;CloseHandle(output);}}
    free(xml);free(snapshot);VirtualFree(image,0,MEM_RELEASE);
    printf("handling_bytes=360 numeric_adapters=%u restored=%u error=%u\n",adapted,restored,error);return (int)error;
}
