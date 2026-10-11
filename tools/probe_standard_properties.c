/* Whole original Standard properties decoder on an owned 360x64 cell matrix.
   Original CSV tokenization/handle generation and CRT numeric parsing excluded.
   Two explicit host numeric adapters; original decoder/setup/lookup/math run. */
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
typedef void (__cdecl *Decode)(void *,int,const char *);
typedef struct {unsigned char *entry;unsigned char bytes[5];DWORD protection;int active;} Adapter;
static double __cdecl number(const char *s,char **end){return strtod(s,end);}
static long __cdecl integer(const char *s,char **end,int base){return strtol(s,end,base);}
static int install(Adapter *a,unsigned char *entry,uintptr_t target){
    a->entry=entry;memcpy(a->bytes,entry,5);DWORD ignored;
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&a->protection))return 0;
    entry[0]=0xe9;unsigned displacement=(unsigned)target-(unsigned)(uintptr_t)(entry+5);memcpy(entry+1,&displacement,4);a->active=1;
    return VirtualProtect(entry,5,a->protection,&ignored)&&FlushInstructionCache(GetCurrentProcess(),entry,5);
}
static int restore(Adapter *a){
    if(!a->active)return 1;DWORD old,ignored;
    if(!VirtualProtect(a->entry,5,PAGE_READWRITE,&old))return 0;
    memcpy(a->entry,a->bytes,5);
    int ok=VirtualProtect(a->entry,5,a->protection,&ignored)&&FlushInstructionCache(GetCurrentProcess(),a->entry,5)&&!memcmp(a->entry,a->bytes,5);
    if(ok)a->active=0;return ok;
}
static LONG diagnostic(EXCEPTION_POINTERS *e){
    fprintf(stderr,"fault=%08lx eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,e->ContextRecord->Eip,(DWORD)e->ExceptionRecord->ExceptionInformation[1]);
    return EXCEPTION_EXECUTE_HANDLER;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    unsigned error=0;Adapter adapters[2]={0};FILE *input=NULL;unsigned char cells[360*64];
    unsigned char saved[0x1000],*page=image+0xe85000-0x400000;DWORD protection=0,ignored;int writable=0,data_restored=1;
    __declspec(align(16)) unsigned char storage[32+0x65c+64];memset(storage,0xcd,sizeof(storage));
    if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
    int valid=fread(cells,1,sizeof(cells),input)==sizeof(cells)&&fgetc(input)==EOF;fclose(input);input=NULL;
    if(!valid){error=6;goto done;}
    for(unsigned i=0;i<360;++i){unsigned char *p=cells+i*64;void *end=memchr(p,0,64);if(!end){error=7;goto done;}
        for(unsigned j=0;j<64;++j)if(p[j]>=128){error=7;goto done;}}
    if(!cells[0]){error=8;goto done;}
    if(!install(&adapters[0],image+0xbf647d-0x400000,(uintptr_t)&number)||!install(&adapters[1],image+0xbf630f-0x400000,(uintptr_t)&integer)){error=9;goto done;}
    memcpy(saved,page,sizeof(saved));if(!VirtualProtect(page,sizeof(saved),PAGE_READWRITE,&protection)){error=10;goto done;}writable=1;
    unsigned *table=(unsigned *)(page+0xe10);table[0]=(unsigned)(uintptr_t)cells;table[1]=1;table[2]=360;table[3]=0;table[4]=0;table[5]=0x2008;
    Decode decode=(Decode)(image+0x9aec60-0x400000);
    __try{decode(storage+32,0,(const char *)cells);}
    __except(diagnostic(GetExceptionInformation())){error=11;}
    for(unsigned i=0;i<32;++i)if(storage[i]!=0xcd)error=12;
    for(unsigned i=32+0x65c;i<sizeof(storage);++i)if(storage[i]!=0xcd)error=12;
    if(!error){
        HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(output==INVALID_HANDLE_VALUE)error=13;
        else{DWORD written=0;if(!WriteFile(output,storage+32,0x65c,&written,NULL)||written!=0x65c)error=14;CloseHandle(output);}
    }
done:
    if(writable){memcpy(page,saved,sizeof(saved));data_restored=VirtualProtect(page,sizeof(saved),protection,&ignored)&&!memcmp(page,saved,sizeof(saved));if(!data_restored)error=15;}
    int restored_integer=restore(&adapters[1]),restored_number=restore(&adapters[0]);if(!restored_integer||!restored_number)error=16;
    if(input)fclose(input);VirtualFree(image,0,MEM_RELEASE);
    printf("properties_bytes=1628 crt_numeric_adapters=2 data_restored=%d code_restored=%d error=%u\n",data_restored,restored_integer&&restored_number,error);
    return (int)error;
}
