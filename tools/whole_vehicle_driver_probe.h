/* Whole A03670 against the installed owned source world. No code replacements.
   Existing helper adapters are included only for their conservative guards;
   none of their authored stage schedules produces the comparison output. */
#include <xmmintrin.h>
typedef void (__cdecl *Update)(void *,float);
#ifdef GRID_VEHICLE_WORLD
#include "vehicle_world_record.h"
#endif
typedef struct {
    unsigned index,delta,body_address,view_address,solver_address,present[4];
    unsigned char body[0x2600],view[0x300],solver[0xdd0],material[4][0x140];
    unsigned char engine_limit[4],diagnostics[32];
#ifdef GRID_WHOLE_OUTER
    unsigned char extra[0x21000-0x2600],physical[0x424];
#endif
#ifdef GRID_HANDLING_QUERIES
    unsigned query_present,query_count,query_records[64][2];
#endif
#ifdef GRID_WHEEL_PRODUCER
    unsigned producer_capacity,producer_used,contact_slots[4][32];
#endif
#ifdef GRID_VEHICLE_WORLD
    VehicleWorldRecord world;
#endif
} DriverRecord;
static unsigned get(const void *p,unsigned o){return getword((const unsigned char *)p+o);}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
#ifdef GRID_CAR_MANAGER
#include "vehicle_manager_probe.h"
#endif
#ifdef GRID_WHEEL_PRODUCER
#include "wheel_producer_probe.h"
#ifdef GRID_VEHICLE_WORLD
#include "vehicle_world_probe.h"
#endif
#endif
#ifdef GRID_WHOLE_OUTER
#include "vehicle_outer_probe.h"
#endif
#ifdef GRID_CAR_JOB
#include "vehicle_handling_probe.h"
#ifdef GRID_HANDLING_QUERIES
#include "handling_query_probe.h"
#endif
/* Observes the actual job's tail-called owned completion object. It does not
   replace any car/physics helper or emulate the original task manager. */
static void __fastcall job_complete(void *self,void *unused){
    (void)unused;unsigned char *p=self;put(p,4,get(p,4)+1);put(p,8,(unsigned)(uintptr_t)self);
}
#endif
#pragma warning(push)
#pragma warning(disable:4505) /* Guard-only inclusion leaves old schedules unused. */
#define GRID_CONTROL
#include "vehicle_controls_probe.h"
#include "vehicle_propulsion_probe.h"
#include "vehicle_prepared_probe.h"
#include "vehicle_driver_prepare_probe.h"
#define tail_guard post_guard
#define tail_run post_run
#include "vehicle_post_force_probe.h"
#undef tail_guard
#undef tail_run
#define tail_guard chassis_guard
#define tail_run chassis_run
#include "vehicle_chassis_probe.h"
#undef tail_guard
#undef tail_run
#pragma warning(pop)
static int driver_fault(EXCEPTION_POINTERS *p){
    fprintf(stderr,"whole_driver_fault=%08lx eip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;
}
static unsigned source_guard(const unsigned char *b,const unsigned char *v,unsigned count){
    const unsigned char *context=b+0x3d0;
    if(get(context,0x5d8)!=4||context[0x70c]||get(context,0x604)||get(context,0x5d4)||get(context,0x608)||get(context,0x60c)||get(b,0x2278))return 66;
    const unsigned ranges[][2]={{0xb20,0xba0},{0x15c0,0x1620},{0x1650,0x1690},{0x2198,0x21c4}};
    for(unsigned i=0;i<4;i++)for(unsigned o=ranges[i][0];o<ranges[i][1];o+=4)if(!isfinite(*(const float *)(b+o)))return 67;
    const unsigned fields[]={0x12ac,0x11e8,0xfcc,0xfe4,0xe74,0x1160,0x1164,0x1168,0x116c,0x1170,0x1174,0x11f4,0x12a8,0x1354,0x137c,0x1efc,0x1f10,0x1f20,0x1f24,0x1f3c,0x1f64,0x1f8c,0x1f90,0x1fa4,0x1fa8,0x1ffc,0x21f8,0x21fc,0x2200};
    for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++)if(!isfinite(*(const float *)(b+fields[i])))return 67;
    for(unsigned j=0;j<10;j++)if(!isfinite(*(const float *)(b+0x11fc+j*4)))return 67;
    if(!isfinite(*(const float *)context)||*(const float *)context<=0||*(const float *)(b+0x1f3c)<=0||*(const float *)(b+0x1ffc)<0||*(const float *)(b+0x137c)<0||*(const float *)(b+0x137c)>=1||*(const float *)(b+0xfcc)<=0||*(const float *)(b+0xfcc)>=1||*(const float *)(b+0x1160)==0||*(const float *)(b+0x21f8)==*(const float *)(b+0x21fc)||*(const float *)(b+0x12a8)<0||*(const float *)(b+0x12a8)>4)return 67;
    /* Whole 9F1E10 overwrites context6F0..6F8 from already guarded car15A8..B0
       before use. A freshly constructed source owner legitimately has zeroes. */
    for(unsigned i=0;i<4;i++){
        const unsigned char *c=context+i*0x170,*w=b+0x1990+i*0x150;
        if(get(c,0x178)||get(c,0x17c)||get(w,0xc0)>count||get(w,0x128)>count)return 68;
        const unsigned offsets[]={0,4,8,12,0x3c,0x40,0x48,0x4c,0x5c,0xe0,0x110,0x140,0x50,0x54,0x58,0x60,0x64,0x68,0x70,0x114,0xb0,0xb4,0xb8,0xbc,0x80,0x104,0xe4,0xec,0x10c,0x11c,0x120,0x124};
        for(unsigned j=0;j<sizeof(offsets)/sizeof(*offsets);j++)if(!isfinite(*(const float *)(w+offsets[j])))return 69;
        if(*(const float *)(w+0xe0)<=0||*(const float *)(w+0x48)>*(const float *)(w+0x4c)||*(const float *)(w+0x4c)<=0||*(const float *)(w+0x48)>*(const float *)(w+0x50)||fabsf(*(const float *)(w+0x48))>=99||fabsf(*(const float *)(w+0x50))>=99||*(const float *)(w+0x64)<0)return 69;
        for(unsigned j=0;j<4;j++)if(!isfinite(*(const float *)(c+0x50+j*4)))return 69;
        const unsigned history[]={0x6c,0x7c,0x80};for(unsigned j=0;j<3;j++)if(!isfinite(*(const float *)(c+history[j])))return 69;
        for(unsigned j=0;j<5;j++)if(!isfinite(*(const float *)(b+0x30c+i*40+j*8))||!isfinite(*(const float *)(b+0x310+i*40+j*8))||*(const float *)(b+0x310+i*40+j*8)<=0)return 69;
    }
    (void)v; /* Earlier driver preparation overwrites view force/history vectors. */
    return 0;
}
static unsigned run_suspension_prepare(unsigned char *image,const wchar_t *request,const wchar_t *output,
    SurfaceRecord *materials,unsigned material_count,unsigned *completed){
    unsigned error=0,protected=0,created=0;FILE *input=NULL,*result=NULL;unsigned header[2]={0};
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xd16000,0xe85000,0xe8e000
#ifdef GRID_CAR_JOB
    ,0xec2000
#ifdef GRID_VEHICLE_WORLD
    ,0xe52000
#endif
#endif
    };
    DWORD protections[sizeof(pages)/sizeof(*pages)];
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 60;memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
#ifdef GRID_CAR_MANAGER
    error=manager_open(image);if(error)goto finish;
#endif
    for(unsigned i=0;i<sizeof(pages)/sizeof(*pages);i++){
        if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=61;goto finish;}++protected;
    }
    if(!materials||!material_count||_wfopen_s(&input,request,L"rb")||!input){error=62;goto finish;}
    const unsigned tag=
#ifdef GRID_CAR_JOB
#ifdef GRID_CAR_MANAGER
#ifdef GRID_WHEEL_PRODUCER
#ifdef GRID_VEHICLE_WORLD
#ifdef GRID_SOURCE_VEHICLE_WORLD
        0x34304654;
#else
        0x33304654;
#endif
#else
        0x32304654;
#endif
#else
        0x31304654;
#endif
#elif defined(GRID_HANDLING_QUERIES)
        0x3230464b;
#else
        0x3130464b;
#endif
#elif defined(GRID_WHOLE_OUTER)
        0x3130464f;
#elif defined(GRID_WHOLE_WRAPPER)
        0x31304651;
#else
        0x31304645;
#endif
    if(fseek(input,0,SEEK_END)||ftell(input)>16*1024*1024||fseek(input,0,SEEK_SET)||fread(header,8,1,input)!=1||header[0]!=tag||!header[1]||header[1]>64){error=63;goto finish;}
    HANDLE h=CreateFileW(output,GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE){error=73;goto finish;}
    int fd=_open_osfhandle((intptr_t)h,_O_BINARY);if(fd<0){CloseHandle(h);error=73;goto finish;}created=1;result=_fdopen(fd,"wb");if(!result){_close(fd);error=73;goto finish;}
    if(fwrite(header,8,1,result)!=1){error=74;goto finish;}
    for(unsigned index=0;index<header[1];index++){
        DriverRecord r;if(fread(&r,sizeof(r),1,input)!=1||r.index!=index){error=65;break;}
        float delta;memcpy(&delta,&r.delta,4);
        if(!isfinite(delta)||delta<0||delta>.1f||(delta>0&&!isfinite(1.f/delta))||r.body[0xadc]||get(r.body,0)!=r.view_address||get(r.view,0)!=r.solver_address||r.solver_address-r.view_address!=0x2b0||memcmp(r.view+0x2b0,r.solver,0x50)){error=66;break;}
#ifndef GRID_WHOLE_WRAPPER
        if(delta==0){error=66;break;}
#else
        const unsigned outer_fields[]={0x2e8,0xe1c,0xe20,0x11f0,0x1224,0x1228,0x122c,0x1230,0x1234,0x1238,0x1648};
        for(unsigned i=0;i<sizeof(outer_fields)/sizeof(*outer_fields);i++)if(!isfinite(*(float *)(r.body+outer_fields[i])))error=66;
        if((*(float *)(r.body+0x122c)>0||*(float *)(r.body+0x1230)>0)&&*(float *)(r.body+0x1224)==*(float *)(r.body+0x1228))error=66;
        if(*(float *)(r.body+0x11f0)>0&&!isfinite((*(float *)(r.body+0xe20)* *(float *)(r.body+0x11f0))*delta))error=66;
        if(error)break;
#endif
        if(!isfinite(*(float *)r.engine_limit)){error=66;break;}
        for(unsigned i=0;i<8;i++)if(!isfinite(*(float *)(r.diagnostics+i*4)))error=66;if(error)break;
#ifdef GRID_WHOLE_OUTER
        __declspec(align(16)) unsigned char body[0x21000],physical[0x430],wrapper[16]={0},component[0x100]={0},group[0x70]={0};
        memcpy(body,r.body,0x2600);memcpy(body+0x2600,r.extra,sizeof(r.extra));memcpy(physical,r.physical,0x424);
#ifdef GRID_CAR_MANAGER
        error=manager_guard(body);if(error)break;
#ifdef GRID_WHEEL_PRODUCER
        error=wheel_input_guard(&r);if(error)break;
#endif
#endif
#ifdef GRID_HANDLING_QUERIES
        unsigned char receiver[HANDLING_QUERY_BYTES],receiver_before[HANDLING_QUERY_BYTES];HandlingQueries queries;
        queries.present=r.query_present;queries.count=r.query_count;memcpy(queries.records,r.query_records,sizeof(queries.records));
        error=query_receiver(&queries,receiver);if(error)break;memcpy(receiver_before,receiver,sizeof(receiver));
#endif
        error=outer_guard(body,physical);if(error)break;
        /* Guard the state seen by the driver using owned scratch synchronization.
           Restore the entry state before the sole result-producing A09A80 call. */
        put(wrapper,8,(unsigned)(uintptr_t)physical);put(body,0xdb0,(unsigned)(uintptr_t)wrapper);
        __try{((void (__fastcall *)(void *,void *))(image+0x9bd7f0-0x400000))(body,NULL);}
        __except(driver_fault(GetExceptionInformation())){error=70;}if(error)break;
        put(body,0xdb0,0);
        if(!outer_finite(physical+0x90,16)||!outer_finite(physical+0x150,16)||!outer_finite(physical+0x3f0,2)){error=33;break;}
#else
        __declspec(align(16)) unsigned char body[0x2600];
        memcpy(body,r.body,0x2600);
#endif
        __declspec(align(16)) unsigned char storage[0x1080],old_material[4][0x140]={0},table[0x20]={0};unsigned entry[3]={0};
        unsigned char *view=storage,*solver=storage+0x2b0;memcpy(view,r.view,0x300);memcpy(solver,r.solver,0xdd0);
        error=source_guard(body,view,material_count);if(error)break;
        for(unsigned i=0;i<4;i++){
            unsigned w=0x1990+i*0x150,token=get(body,w+0xc0);
            if(get(body,w+0x34)!=r.body_address+0x15a0||r.present[i]!=(unsigned)(token!=0)){error=68;break;}
            if(token)memcpy(old_material[i],materials[token-1].data,0xb0);
        }
        if(error)break;
        unsigned guard_stage=1;error=driver_guard(body);if(!error){guard_stage=2;error=controls_guard(body);}
        __declspec(align(16)) unsigned char guarded_view[0x300];memcpy(guarded_view,view,sizeof(guarded_view));memset(guarded_view+0x250,0,64);
        if(!error){guard_stage=3;error=propulsion_guard(body,guarded_view,old_material);}
        if(!error){guard_stage=4;error=prepared_guard(body,old_material,delta);}
        /* Suspension publishes wheel load138 before the later response guard. */
        __declspec(align(16)) unsigned char later_guard[0x2600];memcpy(later_guard,body,sizeof(later_guard));
        for(unsigned i=0;i<4;i++)put(later_guard,0x1990+i*0x150+0x138,0);
        if(!error&&delta>0){guard_stage=5;error=post_guard(later_guard,delta);}if(!error&&delta>0){guard_stage=6;error=chassis_guard(body,delta);}
        if(error){printf("whole_driver_guard_case=%u stage=%u group=%x\n",index,guard_stage,driver_guard_group);break;}
        unsigned driven=get(body,0x18f0);if(!driven||driven>4){error=68;break;}
        for(unsigned i=0;i<driven;i++){
            unsigned p=get(body,0x18d8+i*4),o=p-r.body_address;
            if(o<0x1990||o>=0x1ed0||(o-0x1990)%0x150){error=68;break;}
            for(unsigned j=0;j<i;j++)if(p==get(body,0x18d8+j*4))error=68;
        }
        for(unsigned i=0;i<2;i++){
            unsigned o=get(body,0x21cc+i*4)-r.body_address;
            if(o<0x1990||o>=0x1ed0||(o-0x1990)%0x150)error=68;
        }
        if(get(body,0x21cc)==get(body,0x21d0))error=68;if(error)break;
#ifdef GRID_WHOLE_OUTER
        memcpy(body,r.body,0x2600);memcpy(body+0x2600,r.extra,sizeof(r.extra));memcpy(physical,r.physical,0x424);
        put(wrapper,8,(unsigned)(uintptr_t)physical);put(body,0xdb0,(unsigned)(uintptr_t)wrapper);put(component,0x24,(unsigned)(uintptr_t)body);
        put(physical,0x32c,(unsigned)(uintptr_t)group);
        for(unsigned i=0;i<4;i++)put(physical,0x334+i*4,(unsigned)(uintptr_t)(physical+(i?0x180+i*16:0x1c0)));
        /* Preflight synchronization's temporary SIMD global scratch must not
           leak into the result-producing call. Other per-case inputs follow. */
        memcpy(image+0xe5b000-0x400000,saved+0xe5b000-0x400000,4096);
        put(image,0xe85fcc-0x400000,0);
#endif
        put(body,0,(unsigned)(uintptr_t)view);put(view,0,(unsigned)(uintptr_t)solver);
        for(unsigned i=0;i<driven;i++)put(body,0x18d8+i*4,(unsigned)(uintptr_t)(body+get(r.body,0x18d8+i*4)-r.body_address));
        for(unsigned i=0;i<2;i++)put(body,0x21cc+i*4,(unsigned)(uintptr_t)(body+get(r.body,0x21cc+i*4)-r.body_address));
        for(unsigned i=0;i<4;i++){
            unsigned w=0x1990+i*0x150;put(body,w+0x34,(unsigned)(uintptr_t)(body+0x15a0));
#ifdef GRID_WHEEL_PRODUCER
            unsigned other=get(body,w+0xc4);if(other)put(body,w+0xc4,(unsigned)(uintptr_t)wheel_source_objects[other-2].wrapper);
#endif
            for(unsigned o=0xc0;o<=0x128;o+=0x68){unsigned token=get(body,w+o);put(body,w+o,token?(unsigned)(uintptr_t)materials[token-1].data:0);}
            put(body,0x3d0+i*0x170+0x178,(unsigned)(uintptr_t)(body+w));
        }
        put(body,0x3d0+0x5d4,(unsigned)(uintptr_t)body);put(table,0xc,1);put(table,0x14,(unsigned)(uintptr_t)entry);
        put(image,0xe85fd0-0x400000,(unsigned)(uintptr_t)table);put(image,0xe85ed4-0x400000,0);put(image,0xec2c54-0x400000,0);
        memcpy(image+0xd16348-0x400000,r.engine_limit,4);memcpy(image+0xe85df0-0x400000,r.diagnostics,32);
        float gravity=-9.8100004196167f;memcpy(image+0xe8efd4-0x400000,&gravity,4);
#ifdef GRID_CAR_JOB
        /* Only whole original calls on a fully owned scratch state determine
           the handling preflight. Restore ALL entry buffers and per-case
           globals before the sole result-producing registered job call. */
        __declspec(align(16)) unsigned char saved_body[0x21000],saved_physical[0x430],saved_storage[0x1080];
        unsigned char saved_wrapper[sizeof(wrapper)],saved_component[sizeof(component)],saved_group[sizeof(group)],saved_table[sizeof(table)],saved_entry[sizeof(entry)];
        unsigned char saved_pages[sizeof(pages)/sizeof(*pages)][4096];
        size_t material_bytes=(size_t)material_count*sizeof(*materials);unsigned char *saved_materials=malloc(material_bytes);if(!saved_materials){error=60;break;}memcpy(saved_materials,materials,material_bytes);
        memcpy(saved_body,body,sizeof(body));memcpy(saved_physical,physical,sizeof(physical));memcpy(saved_storage,storage,sizeof(storage));
        memcpy(saved_wrapper,wrapper,sizeof(wrapper));memcpy(saved_component,component,sizeof(component));memcpy(saved_group,group,sizeof(group));memcpy(saved_table,table,sizeof(table));memcpy(saved_entry,entry,sizeof(entry));
        for(unsigned j=0;j<sizeof(pages)/sizeof(*pages);j++)memcpy(saved_pages[j],image+pages[j]-0x400000,4096);
        __try{
#ifdef GRID_CAR_MANAGER
            put(body,0x21c4,0x3f800000);
#endif
            ((void (__fastcall *)(void *,void *))(image+0x9bd7f0-0x400000))(body,NULL);
            ((void (__fastcall *)(void *,void *,float))(image+0xa06a90-0x400000))(body,NULL,delta);
            ((void (__fastcall *)(void *,void *,float,unsigned))(image+0xa03220-0x400000))(body,NULL,delta,0);
        }__except(driver_fault(GetExceptionInformation())){error=70;}
        if(!error){put(body,0xdb0,0);error=handling_guard(body,physical,delta);}
#ifdef GRID_CAR_MANAGER
        if(!error)error=manager_finish_guard(body);
#ifdef GRID_WHEEL_PRODUCER
        if(!error)error=wheel_point_guard(body,physical);
#endif
#endif
        if(memcmp(materials,saved_materials,material_bytes))error=71;
        memcpy(materials,saved_materials,material_bytes);free(saved_materials);
        memcpy(body,saved_body,sizeof(body));memcpy(physical,saved_physical,sizeof(physical));memcpy(storage,saved_storage,sizeof(storage));
        memcpy(wrapper,saved_wrapper,sizeof(wrapper));memcpy(component,saved_component,sizeof(component));memcpy(group,saved_group,sizeof(group));memcpy(table,saved_table,sizeof(table));memcpy(entry,saved_entry,sizeof(entry));
        for(unsigned j=0;j<sizeof(pages)/sizeof(*pages);j++)memcpy(image+pages[j]-0x400000,saved_pages[j],4096);
        if(error)break;
        put(body,MUTABLE+0x214,(unsigned)(uintptr_t)component);
#ifdef GRID_HANDLING_QUERIES
        put(component,0x10,queries.present?(unsigned)(uintptr_t)receiver:0);
#endif
#ifdef GRID_CAR_MANAGER
        __declspec(align(16)) unsigned char taskmanager[0xd0]={0};
        manager_setup(image,body);
#ifdef GRID_WHEEL_PRODUCER
        __declspec(align(16)) WheelOwner wheel_owner;wheel_setup(&wheel_owner,&r,body,physical);
#ifdef GRID_VEHICLE_WORLD
        error=vw_setup(&wheel_owner,&r.world,physical,image,r.delta);if(error)break;
#endif
        unsigned char wheel_before[sizeof(WheelOwner)-sizeof(wheel_owner.slots)];memcpy(wheel_before,&wheel_owner,sizeof(wheel_before));
#else
        if(get(body,0xdc8))put(body,0xdc8,(unsigned)(uintptr_t)wrapper);
#endif
#else
        unsigned job[7]={0},completion_table[6]={0};
        job[1]=(unsigned)(uintptr_t)body;job[2]=r.delta;job[3]=(unsigned)(uintptr_t)completion_table;job[6]=0x125ac07e;
        completion_table[5]=(unsigned)(uintptr_t)job_complete;
#endif
#endif
#ifdef GRID_VEHICLE_WORLD
        for(unsigned world_interval=0;world_interval<r.world.steps;world_interval++){
#endif
        __try{
#ifdef GRID_CAR_JOB
#ifdef GRID_CAR_MANAGER
            ((void (__cdecl *)(float,void *))(image+0xa02b70-0x400000))(delta,taskmanager);
#else
            ((void (__fastcall *)(void *,void *))(image+0xa0a290-0x400000))(job,NULL);
#endif
#elif defined(GRID_WHOLE_OUTER)
            /* Entry has ret8: callee pops the two stack arguments. */
            ((void (__stdcall *)(void *,float))(image+0xa09a80-0x400000))(component,delta);
#else
            ((void (__fastcall *)(void *,void *,float))(image+
#ifdef GRID_WHOLE_WRAPPER
            0xa06a90
#else
            0xa03670
#endif
            -0x400000))(body,NULL,delta);
#endif
        }
        __except(driver_fault(GetExceptionInformation())){error=70;}
        if(error)break;
#ifdef GRID_VEHICLE_WORLD
        if(!manager_retained(image,body,r.delta)){error=71;break;}
        __try{error=vw_step(&wheel_owner,&r.world,physical,image);}__except(driver_fault(GetExceptionInformation())){error=70;}
        if(error)break;
        }
        if(error)break;
#endif
#ifdef GRID_CAR_JOB
#ifdef GRID_CAR_MANAGER
        if(!manager_retained(image,body,r.delta)){error=71;break;}
#ifdef GRID_WHEEL_PRODUCER
#ifdef GRID_VEHICLE_WORLD
        error=vw_output(&wheel_owner,wheel_before,&r,body,physical);if(error)break;
#else
        error=wheel_output(&wheel_owner,wheel_before,&r,body,physical);if(error)break;
#endif
#else
        if(get(body,0xdc8)!=(get(r.body,0xdc8)?(unsigned)(uintptr_t)wrapper:0u)){error=71;break;}
        put(body,0xdc8,get(r.body,0xdc8));
#endif
        for(unsigned j=0;j<sizeof(taskmanager);j++)if(taskmanager[j])error=71;
        if(error)break;
#else
        if(job[0]||job[1]!=(unsigned)(uintptr_t)body||job[2]!=r.delta||job[3]!=(unsigned)(uintptr_t)completion_table||job[4]!=1||job[5]!=(unsigned)(uintptr_t)(job+3)||job[6]!=0x125ac07e||completion_table[5]!=(unsigned)(uintptr_t)job_complete){error=71;break;}
        for(unsigned j=0;j<5;j++)if(completion_table[j])error=71;
#endif
        if(get(body,MUTABLE+0x214)!=(unsigned)(uintptr_t)component){error=71;break;}
#ifdef GRID_HANDLING_QUERIES
        if(get(component,0x10)!=(queries.present?(unsigned)(uintptr_t)receiver:0)||memcmp(receiver,receiver_before,sizeof(receiver))){error=71;break;}
        put(component,0x10,0);
#endif
        put(body,MUTABLE+0x214,0);put(component,0x24,0);for(unsigned j=0;j<sizeof(component);j++)if(component[j])error=71;put(component,0x24,(unsigned)(uintptr_t)body);
        if(!bounded(body+MUTABLE,0,0x60)||!bounded(body+MUTABLE,0x3a0,0x3c4)||!bounded(body+MUTABLE,0x42c,0x430))error=27;
        for(unsigned j=0;j<4;j++)if(!bounded(body+MUTABLE,0x3c8+j*0x18,0x3dc+j*0x18))error=27;
        if(error)break;
#endif
#ifndef GRID_WHOLE_WRAPPER
        if(get(body,0)!=(unsigned)(uintptr_t)view){error=71;break;}
#endif
        /* The wrapper's private temporary view has expired. Never follow it. */
        if(get(view,0)!=(unsigned)(uintptr_t)solver||(delta>0&&get(body,0x3d0+0x608)!=(unsigned)(uintptr_t)table)||get(body,0x3d0+0x5d4)!=(unsigned)(uintptr_t)body){error=71;break;}
        put(body,0,r.view_address);put(view,0,r.solver_address);put(body,0x3d0+0x608,0);put(body,0x3d0+0x5d4,0);
        for(unsigned i=0;i<driven;i++){unsigned o=get(r.body,0x18d8+i*4)-r.body_address;if(get(body,0x18d8+i*4)!=(unsigned)(uintptr_t)(body+o))error=71;put(body,0x18d8+i*4,get(r.body,0x18d8+i*4));}
        for(unsigned i=0;i<2;i++){unsigned o=get(r.body,0x21cc+i*4)-r.body_address;if(get(body,0x21cc+i*4)!=(unsigned)(uintptr_t)(body+o))error=71;put(body,0x21cc+i*4,get(r.body,0x21cc+i*4));}
        for(unsigned i=0;i<4;i++){
            unsigned w=0x1990+i*0x150,c=0x3d0+i*0x170;
            if(get(body,w+0x34)!=(unsigned)(uintptr_t)(body+0x15a0)||get(body,c+0x178)!=(unsigned)(uintptr_t)(body+w)){error=71;break;}
            put(body,w+0x34,get(r.body,w+0x34));put(body,c+0x178,0);
            for(unsigned o=0xc0;o<=0x128;o+=0x68){
                uintptr_t token=get(body,w+o),base=(uintptr_t)materials;
                if(token&&(token<base||token>=base+material_count*sizeof(SurfaceRecord)||(token-base)%sizeof(SurfaceRecord))){error=72;break;}
                put(body,w+o,token?(unsigned)((token-base)/sizeof(SurfaceRecord))+1:0);
            }
#ifdef GRID_WHEEL_PRODUCER
            unsigned shape_token=wheel_token(get(body,w+0xc4),1);if(shape_token==0xffffffff){error=71;break;}put(body,w+0xc4,shape_token);
#else
            put(body,w+0xc4,get(body,w+0xc4)!=0);
#endif
            put(body,w+0xc8,get(body,w+0xc8)!=0);
            unsigned token=get(body,w+0xc0);r.present[i]=token!=0;memset(r.material[i],0,0x140);
            if(token){const unsigned fields[]={0x10,0x14,0x18,0x1c,0x20,0x64};for(unsigned j=0;j<6;j++)memcpy(r.material[i]+fields[j],materials[token-1].data+fields[j],4);r.material[i][0x75]=materials[token-1].data[0x75];}
        }
        if(error)break;
#ifdef GRID_WHOLE_OUTER
        if(get(body,0xdb0)!=(unsigned)(uintptr_t)wrapper||get(wrapper,8)!=(unsigned)(uintptr_t)physical||get(component,0x24)!=(unsigned)(uintptr_t)body||get(body,0xc784)!=(unsigned)(uintptr_t)(image+0x9a0c90-0x400000)){error=71;break;}
        if(get(physical,0x32c)!=(unsigned)(uintptr_t)
#ifdef GRID_VEHICLE_WORLD
           wheel_owner.world
#else
           group
#endif
        ){error=71;break;}
        for(unsigned i=0;i<4;i++)if(get(physical,0x334+i*4)!=(unsigned)(uintptr_t)(physical+(i?0x180+i*16:0x1c0)))error=71;
        if(error)break;
        put(body,0xdb0,0);put(body,0xc784,0x9a0c90);
        put(physical,0x32c,0);for(unsigned i=0;i<4;i++)put(physical,0x334+i*4,0);
        memcpy(r.extra,body+0x2600,sizeof(r.extra));memcpy(r.physical,physical,0x424);
#endif
        memcpy(r.body,body,0x2600);memcpy(r.view,view,0x300);memcpy(r.solver,solver,0xdd0);memcpy(r.engine_limit,image+0xd16348-0x400000,4);memcpy(r.diagnostics,image+0xe85df0-0x400000,32);
        if(fwrite(&r,sizeof(r),1,result)!=1){error=74;break;}++*completed;
    }
    if(!error&&(fgetc(input)!=EOF||ferror(input)))error=65;
finish:
    if(input&&fclose(input))error=77;if(result&&fclose(result))error=77;
    unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t n=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,n))unchanged=0;}
    unsigned restored=1;for(unsigned i=0;i<protected;i++){DWORD ignored;memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&ignored))restored=0;}
#ifdef GRID_CAR_MANAGER
    if(!manager_close(image))restored=0;
    if(memcmp(image+0xc89000-0x400000,saved+0xc89000-0x400000,4096))restored=0;
#endif
    if(!unchanged)error=76;if(!restored)error=75;if(error&&created)_wremove(output);free(saved);
#ifdef GRID_CAR_JOB
#ifdef GRID_CAR_MANAGER
    printf("car_manager_intervals=%u original_dispatch_wait=1 owned_events_consumed=1\n",*completed);
#else
    printf("car_job_completions=%u owned_completion_identity=1\n",*completed);
#endif
#endif
#ifdef GRID_VEHICLE_WORLD
    printf("vehicle_world_steps=%u contacts=%u repeated_contacts=%u empty_steps=%u common_delta=1 authoritative_physical_feedback=1 no_cold_reset=1\n",vw_steps_completed,vw_contacts_produced,vw_repeated_contacts,vw_empty_steps);
#endif
    printf("whole_driver_records=%u error=%u code_unchanged=%u driver_globals_restored=%u\n",*completed,error,unchanged,restored);return error;
}
