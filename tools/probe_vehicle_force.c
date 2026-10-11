/* Whole original selected wheel-force update. Captured bounded owned memory;
   explicit pointer relocation only, no live injection or helper replacement. */
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
#ifdef GRID_PROPULSION
#include <float.h>
#endif
#include "isolated_reference.h"
#ifdef GRID_DRIVER_PREPARE
#include <xmmintrin.h>
#endif
typedef void (__cdecl *Update)(void *,float);
typedef struct {
    unsigned index,delta,body_address,view_address,solver_address,present[4];
    unsigned char body[0x2600],view[0x300],solver[0xdd0],material[4][0x140];
    unsigned char engine_limit[4],diagnostics[32];
} Record;
static unsigned get(const void *p,unsigned offset) {unsigned v;memcpy(&v,(const unsigned char *)p+offset,4);return v;}
static void put(void *p,unsigned offset,unsigned v) {memcpy((unsigned char *)p+offset,&v,4);}
static unsigned exception(EXCEPTION_POINTERS *e) {
    fprintf(stderr,"exception=%08lx instruction=%p",e->ExceptionRecord->ExceptionCode,e->ExceptionRecord->ExceptionAddress);
    if(e->ExceptionRecord->NumberParameters>=2)fprintf(stderr," access=%lu address=%p",(unsigned long)e->ExceptionRecord->ExceptionInformation[0],(void *)e->ExceptionRecord->ExceptionInformation[1]);
    fputc('\n',stderr);return EXCEPTION_EXECUTE_HANDLER;
}
#if defined(GRID_PROPULSION) && !defined(GRID_SHIFT_INITIALIZE)
#ifdef GRID_CONTROL
#include "vehicle_controls_probe.h"
#endif
#include "vehicle_propulsion_probe.h"
#ifdef GRID_PREPARED
#include "vehicle_prepared_probe.h"
#endif
#endif
#ifdef GRID_DRIVER_PREPARE
#include "vehicle_driver_prepare_probe.h"
#endif
#ifdef GRID_SHIFT_INITIALIZE
#include "vehicle_gearing_probe.h"
#endif
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
#ifdef GRID_PROPULSION
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
#endif
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;
    memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xd16000,0xe85000
#ifdef GRID_DRIVER_PREPARE
        ,0xe8e000
#endif
    };
    DWORD protections[sizeof(pages)/sizeof(*pages)];unsigned protected=0,error=0,count=0,completed=0;
#ifdef GRID_PROPULSION
    unsigned created=0;
#endif
    for(unsigned i=0;i<sizeof(pages)/sizeof(*pages);i++){
        if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=5;break;}
        ++protected;
    }
    FILE *input=NULL,*output=NULL;
    if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto finish;}
    unsigned header[2];
    const unsigned tag=
#ifdef GRID_SHIFT_INITIALIZE
        0x31304653;
#elif defined(GRID_DRIVER_PREPARE)
        0x31304644;
#elif defined(GRID_PREPARED)
        0x31304647;
#elif defined(GRID_CONTROL)
        0x31304643;
#elif defined(GRID_PROPULSION)
        0x31304650;
#else
        0x31304657;
#endif
    if(fread(header,sizeof(header),1,input)!=1||header[0]!=tag||!header[1]||header[1]>64){error=7;goto finish;}
    count=header[1];
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){error=8;goto finish;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_BINARY);
    if(descriptor<0){CloseHandle(handle);error=8;goto finish;}
    output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);error=8;goto finish;}
#ifdef GRID_PROPULSION
    created=1;
#endif
    if(fwrite(header,sizeof(header),1,output)!=1){error=9;goto finish;}
#if !defined(GRID_DRIVER_PREPARE) && !defined(GRID_SHIFT_INITIALIZE)
    Update update=(Update)(image+0x9df8b0-0x400000);
#endif
    for(unsigned index=0;index<count;index++){
        Record r;
        if(fread(&r,sizeof(r),1,input)!=1||r.index!=index){error=10;break;}
        float delta;memcpy(&delta,&r.delta,4);
        unsigned driven=get(r.body,0x18f0),gear=get(r.body,0x196c);
        unsigned mode=get(r.body,0x1598);
#ifdef GRID_PROPULSION
        if(mode!=1){error=11;break;}
#else
        if(mode!=1&&mode!=2){error=11;break;}
#endif
        if(!isfinite(delta)||delta<=0||delta>.1f||r.body[0xadc]||
           get(r.body,0x1620)!=4||!driven||driven>4||gear>=16||
           get(r.body,0)!=r.view_address||get(r.view,0)!=r.solver_address||
           r.solver_address-r.view_address!=0x2b0||memcmp(r.view+0x2b0,r.solver,0x50)){error=11;break;}
        if(mode==2){
            float ratio,inertia;
            memcpy(&ratio,r.body+0x18f4+4*gear,4);memcpy(&inertia,r.body+0x17a0,4);
            if(ratio==0)ratio=1;
            inertia=inertia/(ratio*ratio);
            if(!isfinite(inertia)||inertia<=0||!isfinite(1.f/inertia)){error=14;break;}
        }
        __declspec(align(16)) unsigned char body[0x2600],storage[0x1080],material[4][0x140];
        unsigned char *view=storage,*solver=storage+0x2b0;
        memcpy(body,r.body,sizeof(body));memcpy(view,r.view,0x300);memcpy(solver,r.solver,0xdd0);memcpy(material,r.material,sizeof(material));
        put(body,0,(unsigned)(uintptr_t)view);put(view,0,(unsigned)(uintptr_t)solver);
        for(unsigned i=0;i<4;i++){
            unsigned w=0x1990+i*0x150,ptr=get(r.body,w+0xc0),alias=i;
            if(get(r.body,w+0x34)!=r.body_address+0x15a0||r.present[i]!=(unsigned)(ptr!=0)){error=12;break;}
            for(unsigned j=0;j<i;j++)if(ptr&&ptr==get(r.body,0x1990+j*0x150+0xc0)){
                if(memcmp(r.material[i],r.material[j],0x140)){error=12;break;}alias=j;break;
            }
            put(body,w+0x34,(unsigned)(uintptr_t)(body+0x15a0));
            put(body,w+0xc0,ptr?(unsigned)(uintptr_t)material[alias]:0);
        }
        for(unsigned i=0;i<driven;i++){
            unsigned ptr=get(r.body,0x18d8+4*i),offset=ptr-r.body_address;
            if(offset<0x1990||offset>=0x1990+4*0x150||(offset-0x1990)%0x150){error=13;break;}
            for(unsigned j=0;j<i;j++)if(ptr==get(r.body,0x18d8+4*j))error=13;
            put(body,0x18d8+4*i,(unsigned)(uintptr_t)(body+offset));
        }
        if(error)break;
#ifdef GRID_CONTROL
        for(unsigned i=0;i<2;i++){
            unsigned offset=get(r.body,0x21cc+i*4)-r.body_address;
            if(offset<0x1990||offset>=0x1990+4*0x150||(offset-0x1990)%0x150){error=41;break;}
            put(body,0x21cc+i*4,(unsigned)(uintptr_t)(body+offset));
        }
        if(get(body,0x21cc)==get(body,0x21d0))error=41;
        if(error)break;
#endif
#if defined(GRID_PROPULSION) && !defined(GRID_DRIVER_PREPARE) && !defined(GRID_SHIFT_INITIALIZE)
        error=propulsion_guard(body,view,material);if(error)break;
#endif
#ifdef GRID_DRIVER_PREPARE
        error=driver_guard(body);if(error){printf("driver_guard_case=%u group=%x bits=%08x\n",index,driver_guard_group,get(body,driver_guard_group));break;}
#endif
#ifdef GRID_SHIFT_INITIALIZE
        error=gearing_guard(body);if(error)break;
#endif
#ifdef GRID_CONTROL
        error=controls_guard(body);if(error)break;
#endif
#ifdef GRID_PREPARED
        error=prepared_guard(body,material,delta);if(error)break;
#endif
        memcpy(image+0xd16348-0x400000,r.engine_limit,4);
        memcpy(image+0xe85df0-0x400000,r.diagnostics,32);
        __try {
#ifdef GRID_SHIFT_INITIALIZE
            error=gearing_run(image,body);
#elif defined(GRID_DRIVER_PREPARE)
            float gravity=-9.8100004196167f;memcpy(image+0xe8efd4-0x400000,&gravity,4);
            error=driver_run(image,body,view,delta);
#elif defined(GRID_PREPARED)
            error=prepared_run(image,body,view,update,delta);
#elif defined(GRID_PROPULSION)
            error=propulsion_run(image,body,view,update,delta);
#else
            update(body,delta);
#endif
        }
        __except(exception(GetExceptionInformation())){error=20;}
        if(error)break;
        /* Normalize only the explicit input relocations, checking preservation. */
        if(get(body,0)!=(unsigned)(uintptr_t)view||get(view,0)!=(unsigned)(uintptr_t)solver){error=21;break;}
        put(body,0,r.view_address);put(view,0,r.solver_address);
#ifdef GRID_CONTROL
        for(unsigned i=0;i<2;i++){
            unsigned offset=get(r.body,0x21cc+i*4)-r.body_address;
            if(get(body,0x21cc+i*4)!=(unsigned)(uintptr_t)(body+offset)){error=21;break;}
            put(body,0x21cc+i*4,get(r.body,0x21cc+i*4));
        }
        if(error)break;
#endif
        for(unsigned i=0;i<4;i++){
            unsigned w=0x1990+i*0x150;
            if(get(body,w+0x34)!=(unsigned)(uintptr_t)(body+0x15a0)){error=21;break;}
            unsigned ptr=get(r.body,w+0xc0),alias=i;
            for(unsigned j=0;j<i;j++)if(ptr&&ptr==get(r.body,0x1990+j*0x150+0xc0)){alias=j;break;}
            if(get(body,w+0xc0)!=(ptr?(unsigned)(uintptr_t)material[alias]:0)){error=21;break;}
            put(body,w+0x34,get(r.body,w+0x34));put(body,w+0xc0,ptr);
            memcpy(r.material[i],material[alias],0x140);
        }
        for(unsigned i=0;i<driven;i++){
            unsigned offset=get(r.body,0x18d8+4*i)-r.body_address;
            if(get(body,0x18d8+4*i)!=(unsigned)(uintptr_t)(body+offset)){error=21;break;}
            put(body,0x18d8+4*i,get(r.body,0x18d8+4*i));
        }
        if(error)break;
        memcpy(r.body,body,sizeof(body));memcpy(r.view,view,0x300);memcpy(r.solver,solver,0xdd0);
        memcpy(r.engine_limit,image+0xd16348-0x400000,4);memcpy(r.diagnostics,image+0xe85df0-0x400000,32);
        if(fwrite(&r,sizeof(r),1,output)!=1){error=22;break;}++completed;
    }
    if(!error&&fgetc(input)!=EOF)error=23;
finish:
    if(input)fclose(input);if(output&&fclose(output))error=24;
    unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&
        memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    for(unsigned i=0;i<protected;i++){
        memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);DWORD previous;
        if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&previous))error=25;
    }
    if(!unchanged)error=26;
#ifdef GRID_PROPULSION
    if(error&&created)_wremove(argv[3]);
#endif
    free(saved);VirtualFree(image,0,MEM_RELEASE);
    printf("vehicle_force_cases=%u completed=%u code_unchanged=%u error=%u\n",count,completed,unchanged,error);
#ifdef GRID_PREPARED
    printf("prepared_drag_changes=%u roughness_rng=%u roughness_changes=%u inertia_changes=%u\n",prepared_drag_changes,prepared_roughness_rng,prepared_roughness_changes,prepared_inertia_changes);
#endif
    return error;
}
