/* Whole9BD7F0 with owned wrapper/body and zero deformation-grid dimensions. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <io.h>
#include <fcntl.h>
#include "isolated_reference.h"
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static int finite(const void *p,unsigned n){for(unsigned i=0;i<n;i++)if(!isfinite(((const float *)p)[i]))return 0;return 1;}
#ifdef GRID_ACTIVE_DEFORMATION
#include "active_deformation_probe.h"
#endif
static unsigned guard(const unsigned char *v,const unsigned char *p){
    if(get(v,0xdb0)||get(v,0xc7ec)>=10||!finite(p+0x1d0,24)||!finite(v+0xc7c4,10)||!finite(v+0xc7b4,4))return 30;
#ifdef GRID_ACTIVE_DEFORMATION
    if(!grid_deformation_guard(v))return 30;
#else
    if(get(v,0xc714)||get(v,0xc718)||get(v,0xc71c))return 30;
#endif
    const unsigned settings[]={0xd198,0xd1ac,0xd1b0,0xce04,0xd1c8,0xd1e0,0xd1f8,0xd210,0xd19c,0xd1c4,0xd1dc,0xd1f4,0xd20c,0xd1bc,0xd1d4,0xd1ec,0xd204,0xd1a8,0xd194,0xd1a0,0xd1a4,0xfa8,0xfac,0xfb0,0xfb4,0x1560};
    for(unsigned i=0;i<sizeof(settings)/sizeof(*settings);i++)if(!finite(v+settings[i],1))return 31;
    const float scale=*(const float *)(v+0x1560),mass=*(const float *)(v+0xfa8)*scale;
    const float a=*(const float *)(v+0xfac)*scale,b=*(const float *)(v+0xfb0)*scale,c=*(const float *)(v+0xfb4)*scale;
    const float determinant=(b*c)*a;
    if(!isfinite(mass)||mass<=0||!isfinite(1.f/mass)||!isfinite(a)||a<=0||!isfinite(b)||b<=0||!isfinite(c)||c<=0||!isnormal(determinant)||determinant<=0)return 32;
    return 0;
}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"vehicle_sync_fault=%08lx eip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
    DWORD previous=0;unsigned error=0,completed=0,created=0,protected=0;
    if(!VirtualProtect(image+0xe5b000-0x400000,4096,PAGE_READWRITE,&previous))error=5;else protected=1;
    FILE *input=NULL,*output=NULL;unsigned header[2]={0};
    if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto finish;}
    if(fseek(input,0,SEEK_END)||ftell(input)>8*1024*1024||fseek(input,0,SEEK_SET)||fread(header,8,1,input)!=1||header[0]!=0x31305356||!header[1]||header[1]>64){error=7;goto finish;}
    HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE){error=8;goto finish;}
    int fd=_open_osfhandle((intptr_t)h,_O_BINARY);if(fd<0){CloseHandle(h);error=8;goto finish;}
    output=_fdopen(fd,"wb");if(!output){_close(fd);error=8;goto finish;}created=1;
    if(fwrite(header,8,1,output)!=1){error=9;goto finish;}
    for(unsigned i=0;i<header[1];i++){
        __declspec(align(16)) unsigned char vehicle[0xd240],physical[0x430],wrapper[16]={0};
        if(fread(vehicle,0xd240,1,input)!=1||fread(physical,0x424,1,input)!=1){error=10;break;}
        error=guard(vehicle,physical);if(error)break;
        put(wrapper,8,(unsigned)(uintptr_t)physical);put(vehicle,0xdb0,(unsigned)(uintptr_t)wrapper);
        __try{((void (__fastcall *)(void *,void *))(image+0x9bd7f0-0x400000))(vehicle,NULL);}__except(fault(GetExceptionInformation())){error=20;}
        if(error)break;
        if(get(vehicle,0xdb0)!=(unsigned)(uintptr_t)wrapper||get(wrapper,8)!=(unsigned)(uintptr_t)physical){error=21;break;}put(vehicle,0xdb0,0);
        if(!finite(physical+0x90,16)||!finite(physical+0x150,16)||!finite(physical+0x3f0,2)){error=33;break;}
        if(fwrite(vehicle,0xd240,1,output)!=1||fwrite(physical,0x424,1,output)!=1){error=22;break;}++completed;
    }
    if(!error&&fgetc(input)!=EOF)error=23;
finish:
    if(input)fclose(input);if(output&&fclose(output))error=24;
    unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    if(protected){memcpy(image+0xe5b000-0x400000,saved+0xe5b000-0x400000,4096);DWORD ignored;if(!VirtualProtect(image+0xe5b000-0x400000,4096,previous,&ignored))error=25;}
    if(!unchanged)error=26;if(error&&created)_wremove(argv[3]);free(saved);VirtualFree(image,0,MEM_RELEASE);
    printf("vehicle_sync_cases=%u completed=%u code_unchanged=%u globals_restored=1 error=%u\n",header[1],completed,unchanged,error);return error;
}
