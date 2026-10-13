/* Whole unchanged A03220, owned full car and linked physical body. */
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
static unsigned get(const void *p,unsigned o){unsigned x;memcpy(&x,(const char *)p+o,4);return x;}
static void put(void *p,unsigned o,unsigned x){memcpy((char *)p+o,&x,4);}
static int finite(const void *p,unsigned n){for(unsigned i=0;i<n;i++)if(!isfinite(((const float *)p)[i]))return 0;return 1;}
static unsigned guard(const unsigned char *v,float delta){
    if(!isfinite(delta)||delta<0||delta>.1f)return 30;
    unsigned independent=get(v,0x1554)==0;
    if(get(v,0xdb0)||get(v,0x1620)!=4||get(v,0xdd4)||v[0xc7f2]||get(v,0xba0)!=(independent?2u:1u)||v[0x20fb4]!=independent)return 31;
#ifdef GRID_DEFORMATION_PUBLICATION
    if(v[4]>1||get(v,0xc8a4)||get(v,0xd004))return 31;
    if(v[4]){
      if(get(v,0xc7f8)!=0xd240||!finite(v+0xee40,1)||fabsf(*(const float *)(v+0xee40))>1000)return 31;
      for(unsigned i=0;i<12;i++)if(get(v,0xcc10+i*32)>2||!finite(v+0xcc20+i*32,4))return 31;
      for(unsigned i=0;i<4;i++)if(!finite(v+0xcd90+i*4,1)||*(const float *)(v+0xcd90+i*4)<=0||!finite(v+0xcda0+i*16,4))return 31;
      if(v[0xc7ad]){unsigned nx=get(v,0xc714),ny=get(v,0xc718),nz=get(v,0xc71c);
        if(nx<2||nx>16||ny<2||ny>16||nz<2||nz>16||nx*ny*nz>400||!finite(v+0xc720,1)||*(const float *)(v+0xc720)<=0||!finite(v+0xc730,8))return 31;
        for(unsigned i=0;i<nx*ny*nz;i++)if(!finite(v+0x4880+i*80,4))return 31;
      }
    }
#else
    if(v[4]||get(v,0xc7a0)==1)return 31;
#endif
    for(unsigned o=0xdb8;o<0xdc8;o+=4)if(get(v,o))return 31;
    if(!finite(v+0xc798,1)||!finite(v+0xc790,1)||!finite(v+0x15c0,3)||!finite(v+0x15d0,3))return 32;
    return 0;
}
#ifdef GRID_HANDLING_PUBLICATION
#include "vehicle_handling_probe.h"
#endif
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"vehicle_publication_fault=%08lx eip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xe85000};DWORD previous[5];unsigned error=0,completed=0,created=0,protected=0;
    for(unsigned i=0;i<5;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,previous+i)){error=5;break;}++protected;}
    FILE *input=NULL,*output=NULL;unsigned header[2]={0};
    if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto finish;}
    unsigned magic=0x31305056;
#ifdef GRID_DEFORMATION_PUBLICATION
    magic=0x31304456;
#endif
#ifdef GRID_HANDLING_PUBLICATION
    magic=0x31304e56;
#endif
    if(fseek(input,0,SEEK_END)||ftell(input)>16*1024*1024||fseek(input,0,SEEK_SET)||fread(header,8,1,input)!=1||header[0]!=magic||!header[1]||header[1]>64){error=7;goto finish;}
    HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE){error=8;goto finish;}
    int fd=_open_osfhandle((intptr_t)h,_O_BINARY);if(fd<0){CloseHandle(h);error=8;goto finish;}
    output=_fdopen(fd,"wb");if(!output){_close(fd);error=8;goto finish;}created=1;
    if(fwrite(header,8,1,output)!=1){error=9;goto finish;}
    put(image,0xe85fcc-0x400000,0);
    for(unsigned i=0;i<header[1];i++){
#ifdef GRID_DEFORMATION_PUBLICATION
        __declspec(align(16)) unsigned char vehicle_buffer[32+0x21000+32],physical_buffer[32+0x424+44],wrapper[16]={0};float delta;
        unsigned char *vehicle=vehicle_buffer+32,*physical=physical_buffer+32;
        memset(vehicle_buffer,0xcd,sizeof(vehicle_buffer));memset(physical_buffer,0xcd,sizeof(physical_buffer));
#else
        __declspec(align(16)) unsigned char vehicle[0x21000],physical[0x430],wrapper[16]={0};float delta;
#endif
        if(fread(&delta,4,1,input)!=1||fread(vehicle,0x21000,1,input)!=1||fread(physical,0x424,1,input)!=1){error=10;break;}
        error=guard(vehicle,delta);if(error)break;
#ifdef GRID_HANDLING_PUBLICATION
        __declspec(align(16)) unsigned char component[0x100]={0},prepared[0x424];
        memcpy(prepared,physical,sizeof(prepared));memcpy(prepared+0x210,vehicle+0x15d0,16);memcpy(prepared+0x220,vehicle+0x15c0,16);put(prepared,0x21c,0);put(prepared,0x22c,0);
        error=handling_guard(vehicle,prepared,delta);if(error)break;
        put(component,0x24,(unsigned)(uintptr_t)vehicle);put(vehicle,MUTABLE+0x214,(unsigned)(uintptr_t)component);
#endif
        put(wrapper,8,(unsigned)(uintptr_t)physical);put(vehicle,0xdb0,(unsigned)(uintptr_t)wrapper);
#ifdef GRID_DEFORMATION_PUBLICATION
        unsigned settings_role=get(vehicle,0xc7f8);
        if(vehicle[4])put(vehicle,0xc7f8,(unsigned)(uintptr_t)(vehicle+0xd240));
#endif
        unsigned notify=0;
#ifdef GRID_HANDLING_PUBLICATION
        notify=1;
#endif
        __try{((void (__fastcall *)(void *,void *,float,unsigned))(image+0xa03220-0x400000))(vehicle,NULL,delta,notify);}__except(fault(GetExceptionInformation())){error=20;}
        if(error)break;
        if(get(vehicle,0xdb0)!=(unsigned)(uintptr_t)wrapper||get(wrapper,8)!=(unsigned)(uintptr_t)physical||get(vehicle,0xc784)!=(unsigned)(uintptr_t)(image+0x9a0c90-0x400000)){error=21;break;}
        put(vehicle,0xdb0,0);put(vehicle,0xc784,0x9a0c90); /* Checked generated code-pointer relocation. */
#ifdef GRID_DEFORMATION_PUBLICATION
        if(vehicle[4]&&get(vehicle,0xc7f8)!=(unsigned)(uintptr_t)(vehicle+0xd240)){error=21;break;}
        put(vehicle,0xc7f8,settings_role);
        for(unsigned j=0;j<32;j++)if(vehicle_buffer[j]!=0xcd||vehicle_buffer[32+0x21000+j]!=0xcd||physical_buffer[j]!=0xcd||physical_buffer[32+0x424+j]!=0xcd)error=21;
        if(error)break;
#endif
#ifdef GRID_HANDLING_PUBLICATION
        if(get(vehicle,MUTABLE+0x214)!=(unsigned)(uintptr_t)component||get(component,0x24)!=(unsigned)(uintptr_t)vehicle){error=21;break;}
        put(vehicle,MUTABLE+0x214,0);put(component,0x24,0);for(unsigned j=0;j<sizeof(component);j++)if(component[j])error=21;
        if(!bounded(vehicle+MUTABLE,0,0x60)||!bounded(vehicle+MUTABLE,0x3a0,0x3c4)||!bounded(vehicle+MUTABLE,0x42c,0x430))error=27;
        for(unsigned j=0;j<4;j++)if(!bounded(vehicle+MUTABLE,0x3c8+j*0x18,0x3dc+j*0x18))error=27;
        if(error)break;
#endif
        if(fwrite(&delta,4,1,output)!=1||fwrite(vehicle,0x21000,1,output)!=1||fwrite(physical,0x424,1,output)!=1){error=22;break;}++completed;
    }
    if(!error&&fgetc(input)!=EOF)error=23;
finish:
    if(input)fclose(input);if(output&&fclose(output))error=24;
    unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,4096,previous[i],&ignored))error=25;}
#ifdef GRID_DEFORMATION_PUBLICATION
    if(memcmp(image,saved,pe->OptionalHeader.SizeOfImage))unchanged=0;
#endif
    if(!unchanged)error=26;if(error&&created)_wremove(argv[3]);free(saved);VirtualFree(image,0,MEM_RELEASE);
    printf("vehicle_publication_cases=%u completed=%u code_unchanged=%u globals_restored=1 error=%u\n",header[1],completed,unchanged,error);return error;
}
