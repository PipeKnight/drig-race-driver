/* Whole original B13E10 body advance then B7B850 supplied-contact frame, PFR3. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include "isolated_reference.h"
#ifndef GRID_CONTACT_GROUPS
__declspec(noinline) static unsigned invoke(void *function,void *manager,void *group,unsigned delta){
 unsigned result;
 __asm { mov ecx,manager
         push delta
         push group
         call function
         mov result,eax }
 return result;
}
#endif
#if !defined(GRID_WORLD_ADVANCE) && !defined(GRID_CONTACT_GROUPS)
__declspec(noinline) static void advance(void *function,void *body,unsigned delta){
 __asm { mov ecx,body
         push 0
         push body
         push delta
         call function }
}
#endif
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float v;memcpy(&v,(const unsigned char *)p+4*i,4);if(!isfinite(v))return 0;}return 1;}
#ifdef GRID_CONTACT_GROUPS
static unsigned owned_token(unsigned address,unsigned char *base,unsigned stride,unsigned count,unsigned *error){
 if(!address)return 0;for(unsigned i=0;i<count;i++)if(address==(unsigned)(uintptr_t)(base+i*stride))return i+1;*error=10;return 0;
}
#endif
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"physical_tick_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 unsigned pages[5]={0xe58000,0xe52000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[5]={0};unsigned char saved[5][4096];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<5;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,&protections[i])){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=
#ifdef GRID_WHOLE_PHYSICAL_WORLD
 0x34535747
#elif defined(GRID_OWNED_SHAPES) && defined(GRID_GROUPED_TICK)
 0x34535447
#elif defined(GRID_OWNED_SHAPES)
 0x34534647
#elif defined(GRID_GROUPED_TICK)
 0x33525447
#elif defined(GRID_CONTACT_GROUPS)
 0x33524647
#elif defined(GRID_WORLD_ADVANCE)
 0x33525750
#else
 0x33524650
#endif
||!read(input,&total,4)||!total||total>1024))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  unsigned h[6],cache[15],ends[4],modes[64],shape_ids[64][2],material_ids[64][2],bodyptrs[16],all_contacts[64],pool_single[64],pool_two[64],ns=0,nt=0;
  #ifdef GRID_WORLD_ADVANCE
  unsigned active_next[16],update_next[16]={0},last=0;
  __declspec(align(16)) unsigned char world[0x100]={0};unsigned update_list[2]={0},lists[1]={(unsigned)(uintptr_t)update_list};
#endif
#ifdef GRID_CONTACT_GROUPS
  unsigned active_next[16],body_ids[64][2],manager_ptrs[1],state[11],ref_limits[256]={0};
  __declspec(align(16)) unsigned char world[0x810]={0},producer[0xc40]={0};
  unsigned char world_before[0x810],producer_before[0xc40];
#ifdef GRID_WHOLE_PHYSICAL_WORLD
  __declspec(align(16)) unsigned char worker[0x48]={0},worker_before[0x48],broadphase[0x100]={0},world_wrapper[16]={0};
  __declspec(align(16)) unsigned char worker_contacts[128]={0},worker_pairs[128]={0};
  unsigned worker_ptrs[1]={(unsigned)(uintptr_t)worker};
#endif
#ifdef GRID_GROUPED_TICK
  unsigned update_list[2]={0},lists[1]={(unsigned)(uintptr_t)update_list},last_active=0,update_next[16]={0};
#endif
#endif
  float parameters[11],correction[4];unsigned correction_iterations;
#ifdef GRID_OWNED_SHAPES
  enum { MAX_SHAPES=513 };
#else
  enum { MAX_SHAPES=2 };
#endif
  unsigned shape_count=MAX_SHAPES;
  unsigned char shape_flags[MAX_SHAPES][4]={0};
  __declspec(align(16)) unsigned char bodies[16][0x430]={0},contacts[64][0x80]={0},shapes[MAX_SHAPES][0x240]={0},manager[0x240]={0},group[0x90]={0},rows[256][192];
  __declspec(align(16)) float materials[8][16]={0},inertia[16][16],mass[16],workspace[16][8];
  unsigned char manager_before[0x100];
  if(!read(input,h,24)||!h[0]||h[0]>16||!h[1]||h[1]>8||h[2]>64||!h[3]||h[3]>256||h[4]>h[3]||h[5]>32||!read(input,parameters,44)||!finite_words(parameters,11)||parameters[0]<=0||parameters[0]>.25f||!read(input,ends,16)||!read(input,cache,60)){error=8;break;}
  if(!read(input,&correction_iterations,4)||correction_iterations>32||!read(input,correction,16)||!finite_words(correction,4)){error=8;break;}
  if((cache[0]!=0xffffffff&&cache[0]>=h[1])||(cache[1]!=0xffffffff&&cache[1]>=h[1])||((cache[0]==0xffffffff)!=(cache[1]==0xffffffff))||!finite_words(cache+2,13)){error=8;break;}
#ifdef GRID_OWNED_SHAPES
  if(!read(input,&shape_count,4)||!shape_count||shape_count>MAX_SHAPES||!read(input,shape_flags,shape_count*4)){error=8;break;}
  for(unsigned s=0;s<shape_count;s++){
   for(unsigned k=0;k<4;k++)if(shape_flags[s][k]>1)error=8;
   shapes[s][0x216]=shape_flags[s][0];shapes[s][0x217]=shape_flags[s][1];shapes[s][0x229]=shape_flags[s][2];shapes[s][0x230]=shape_flags[s][3];
  }
  if(error)break;
#else
  (void)shape_flags;
#endif
  for(unsigned j=0;j<h[0];j++){
   unsigned char *b=bodies[j];float properties[17];
   if(!read(input,b,0x424)||get(b,0x334)!=0x1c0||!finite_words(b+0x110,16)||!finite_words(b+0x1c0,4)||!finite_words(b+0x210,8)||!finite_words(b+0x240,12)||!finite_words(b+0x3f4,1)||!read(input,properties,68)||!finite_words(properties,17)||!read(input,workspace[j],32)||!finite_words(workspace[j],8)){error=8;break;}
   for(unsigned o=0x40c;o<=0x410;o++)if(b[o]>1)error=8;
#ifdef GRID_WHOLE_PHYSICAL_WORLD
   if(b[0x415]){error=8;break;}
#endif
   const unsigned pointers[]={0x2fc,0x320,0x324,0x328,0x32c,0x338,0x33c,0x340,0x418,0x41c,0x420};
   for(unsigned k=0;k<11;k++)if(get(b,pointers[k]))error=8;
   if(get(b,0x400)||b[0x390]>1||!finite_words(b+0x90,16)||!finite_words(b+0x150,16)||!finite_words(b+0x190,16)||!finite_words(b+0x230,4)||!finite_words(b+0x380,4)||!finite_words(b+0x3a0,23))error=8;
   #ifdef GRID_WORLD_ADVANCE
   if(get(b,0x300)>h[0]||get(b,0x318)){error=8;break;}active_next[j]=get(b,0x300);
#endif
#ifdef GRID_CONTACT_GROUPS
   const unsigned cold[]={0,4,8,0xc,0x10,0x14,0x18,0x1c,0x28,0x2f0,0x2f4,0x304,0x308,0x30c,0x318};
   for(unsigned k=0;k<sizeof(cold)/sizeof(*cold);k++)if(get(b,cold[k]))error=8;
   if(get(b,0x300)>h[0]||b[0x2c]>1||b[0x2d]||b[0x412]||b[0x413]||get(b,0x3b4)!=0x7f7fffff){error=8;break;}
   active_next[j]=get(b,0x300);put(b,0x32c,(unsigned)(uintptr_t)world);
#else
   put(b,0x32c,(unsigned)(uintptr_t)group);
   put(b,0x2fc,j+1<h[0]?(unsigned)(uintptr_t)bodies[j+1]:0);
#endif
   put(b,0x334,(unsigned)(uintptr_t)(b+0x1c0));
   for(unsigned k=0;k<3;k++)put(b,0x338+k*4,(unsigned)(uintptr_t)(b+0x190+k*16));
   mass[j]=properties[0];memcpy(inertia[j],properties+1,64);bodyptrs[j]=(unsigned)(uintptr_t)b;
  }
  for(unsigned j=0;!error&&j<h[1];j++)if(!read(input,materials[j],52)||!finite_words(materials[j],13))error=8;
  for(unsigned j=0;!error&&j<h[2];j++){
   unsigned char *c=contacts[j];
   if(!read(input,modes+j,4)||!read(input,c,128)||get(c,0x1c)>=h[0]||get(c,4)!=get(c,0x1c)||get(c,8)!=get(c,0x20)||get(c,0x30)>=h[1]||get(c,0x34)>=h[1]||get(c,0x2c)>shape_count||get(c,0x28)>shape_count||c[0xe]>1||c[0x70]||c[0x73]>1||!finite_words(c+0x38,2)||!finite_words(c+0x50,8)){error=8;break;}
#ifdef GRID_OWNED_SHAPES
   if(modes[j]>2||(modes[j]==2?get(c,8)!=0xffffffff:get(c,8)>=h[0])||(c[0x73]&&(!get(c,0x28)||!get(c,0x2c)))){error=8;break;}
#else
   if(modes[j]>1||get(c,0x20)>=h[0]){error=8;break;}
#endif
   float previous;memcpy(&previous,c+0x3c,4);if(previous<0||get(c,0x10)||c[0xc]!=(modes[j]?0:1)||c[0xd]||c[0xf]!=1){error=8;break;}
   shape_ids[j][0]=get(c,0x2c);shape_ids[j][1]=get(c,0x28);material_ids[j][0]=get(c,0x30);material_ids[j][1]=get(c,0x34);
#ifdef GRID_CONTACT_GROUPS
   body_ids[j][0]=get(c,4);body_ids[j][1]=get(c,8);
#ifndef GRID_OWNED_SHAPES
   if(c[0x73]||shape_ids[j][0]||shape_ids[j][1]){error=8;break;}
#endif
   if(modes[j]==1&&body_ids[j][0]==body_ids[j][1]){error=8;break;}
   for(unsigned k=0;k<4;k++){float x,y;memcpy(&x,bodies[body_ids[j][0]]+0x1c0+4*k,4);memcpy(&y,c+0x50+4*k,4);if(!isfinite(x+y))error=8;}
#endif
   put(c,4,bodyptrs[get(c,0x1c)]);put(c,8,modes[j]==2?0:bodyptrs[get(c,0x20)]);
   put(c,0x30,(unsigned)(uintptr_t)materials[material_ids[j][0]]);put(c,0x34,(unsigned)(uintptr_t)materials[material_ids[j][1]]);
   put(c,0x2c,shape_ids[j][0]?(unsigned)(uintptr_t)shapes[shape_ids[j][0]-1]:0);put(c,0x28,shape_ids[j][1]?(unsigned)(uintptr_t)shapes[shape_ids[j][1]-1]:0);
#ifndef GRID_CONTACT_GROUPS
   put(c,0x10,j+1<h[2]?(unsigned)(uintptr_t)contacts[j+1]:0);
#endif
   if(modes[j]==1)pool_two[nt++]=(unsigned)(uintptr_t)c;else pool_single[ns++]=(unsigned)(uintptr_t)c;
  }
  #ifdef GRID_WORLD_ADVANCE
  if(!error){
   unsigned visited[16]={0},cursor=0,count=0,previous=0;
   for(;;){
    if(cursor>=h[0]||visited[cursor]){error=8;break;}
    visited[cursor]=1;++count;update_next[cursor]=previous;previous=cursor+1;last=cursor;
    if(!active_next[cursor])break;cursor=active_next[cursor]-1;
   }
   if(count!=h[0])error=8;
   if(!error){
    for(unsigned j=0;j<h[0];j++)put(bodies[j],0x300,active_next[j]?bodyptrs[active_next[j]-1]:0);
    update_list[1]=(unsigned)(uintptr_t)group;put(world,8,(unsigned)(uintptr_t)lists);put(world,0xc,1);put(world,0x60,i%9);put(world,0x3c+(i%9)*4,bodyptrs[0]);
   }
  }
#endif
#ifdef GRID_CONTACT_GROUPS
  if(!error){
   unsigned visited[16]={0},cursor=0,count=0,last=0,previous=0;
   for(;;){
    if(cursor>=h[0]||visited[cursor]){error=8;break;}visited[cursor]=1;++count;last=cursor;
    put(bodies[cursor],0x304,previous);previous=bodyptrs[cursor];
    if(!active_next[cursor])break;cursor=active_next[cursor]-1;
   }
   if(count!=h[0]||h[4]<h[2]*4||!h[5])error=8;
   if(!error){
    for(unsigned j=0;j<h[0];j++)put(bodies[j],0x300,active_next[j]?bodyptrs[active_next[j]-1]:0);
    manager_ptrs[0]=(unsigned)(uintptr_t)manager;put(world,4,1);put(world,0x10,(unsigned)(uintptr_t)manager_ptrs);put(world,0x18,(unsigned)(uintptr_t)producer);
    put(world,0x3c,bodyptrs[0]);put(world,0x44,bodyptrs[last]);put(world,0x64,1);put(producer,0x91c,(unsigned)(uintptr_t)contacts);put(producer,0x924,h[2]);
    put(manager,0,(unsigned)(uintptr_t)(image+0xd42654-0x400000));
#ifdef GRID_GROUPED_TICK
    update_list[1]=(unsigned)(uintptr_t)world;put(world,8,(unsigned)(uintptr_t)lists);put(world,0xc,1);last_active=last;
    for(unsigned j=0;j<h[0];j++)if(active_next[j])update_next[active_next[j]-1]=j+1;
#endif
#ifdef GRID_WHOLE_PHYSICAL_WORLD
    put(world,0x84,(unsigned)(uintptr_t)broadphase);put(world_wrapper,4,(unsigned)(uintptr_t)world);
    put(producer,0xa80,(unsigned)(uintptr_t)world);put(producer,0x928,(unsigned)(uintptr_t)worker_ptrs);put(producer,0x92c,1);
    put(producer,0xa78,0x1234);put(producer,0xa7c,0x5678);
    put(worker,0xc,(unsigned)(uintptr_t)worker_pairs);put(worker,0x18,(unsigned)(uintptr_t)worker_contacts);
    memcpy(worker_before,worker,sizeof(worker));
#endif
    memcpy(world_before,world,sizeof(world));memcpy(producer_before,producer,sizeof(producer));
   }
  }
#endif
  if(error||!read(input,rows,h[3]*192)){error=8;break;}
#ifndef GRID_OWNED_SHAPES
  shapes[1][0x216]=1;
#endif
  for(unsigned j=0;j<2;j++)cache[j]=cache[j]==0xffffffff?0:(unsigned)(uintptr_t)materials[cache[j]];
  memcpy(manager+0x20,cache,60);put(manager,0x68,16);put(manager,0x6c,64);memcpy(manager+0x10,parameters+1,16);put(manager,0x70,h[4]);put(manager,0x80,(unsigned)(uintptr_t)workspace);put(manager,0x84,(unsigned)(uintptr_t)inertia);put(manager,0x88,(unsigned)(uintptr_t)mass);put(manager,0x8c,(unsigned)(uintptr_t)rows);put(manager,0x90,(unsigned)(uintptr_t)bodyptrs);put(manager,0x98,(unsigned)(uintptr_t)pool_single);put(manager,0xa0,(unsigned)(uintptr_t)pool_two);put(manager,0xa8,h[0]);put(manager,0xac,ns);put(manager,0xb4,nt);
  put(manager,0xc4,ends[0]);put(manager,0xbc,ends[1]);put(manager,0xc8,ends[2]);put(manager,0xc0,ends[3]);memcpy(manager+0xcc,parameters+5,16);put(manager,0xdc,h[5]);memcpy(image+0xe52f24-0x400000,parameters+9,8);
  memcpy(manager+0xe0,correction,12);memcpy(manager+0xf0,correction+3,4);put(manager,0xf4,correction_iterations);memcpy(manager_before,manager,256);
  put(manager,0x94,(unsigned)(uintptr_t)all_contacts);put(manager_before,0x94,(unsigned)(uintptr_t)all_contacts);
  put(group,0,h[2]?(unsigned)(uintptr_t)contacts[0]:0);put(group,8,(unsigned)(uintptr_t)bodies[0]);put(group,0x20,h[0]);put(group,0x24,h[2]);
  unsigned success=0;
  __try{
#ifdef GRID_CONTACT_GROUPS
#ifdef GRID_WHOLE_PHYSICAL_WORLD
   ((void (__fastcall *)(void *,void *,unsigned))(image+0xad8d70-0x400000))(world,NULL,*(unsigned *)parameters);
   ((void (__fastcall *)(void *,void *))(image+0xa727a0-0x400000))(world_wrapper,NULL);
#else
#ifdef GRID_GROUPED_TICK
   ((void (__fastcall *)(void *,void *,unsigned))(image+0xad7040-0x400000))(world,NULL,*(unsigned *)parameters);
#endif
   ((void (__fastcall *)(void *,void *))(image+0xad6c40-0x400000))(world,NULL);
   ((void (__fastcall *)(void *,void *,unsigned))(image+0xb77310-0x400000))(manager,NULL,*(unsigned *)parameters);
#endif
#elif defined(GRID_WORLD_ADVANCE)
   ((void (__fastcall *)(void *,void *,unsigned))(image+0xad7040-0x400000))(world,NULL,*(unsigned *)parameters);
#else
   for(unsigned j=0;j<h[0];j++)advance(image+0xb13e10-0x400000,bodies[j],*(unsigned *)parameters);
#endif
#ifndef GRID_CONTACT_GROUPS
   success=invoke(image+0xb7b850-0x400000,manager,group,*(unsigned *)parameters);
#endif
  }
  __except(fault(GetExceptionInformation())){fprintf(stderr,"case=%u\n",i);error=9;}
  if(error)break;
  #ifdef GRID_WORLD_ADVANCE
  if(update_list[0]!=bodyptrs[last]||update_list[1]!=(unsigned)(uintptr_t)group||get(world,0x3c+(i%9)*4)!=bodyptrs[0]){error=10;break;}
  for(unsigned j=0;j<h[0];j++){
   if(get(bodies[j],0x300)!=(active_next[j]?bodyptrs[active_next[j]-1]:0)||get(bodies[j],0x318)!=(update_next[j]?bodyptrs[update_next[j]-1]:0)){error=10;break;}
   put(bodies[j],0x300,active_next[j]);put(bodies[j],0x318,update_next[j]);
  }
  if(error)break;
#endif
#ifdef GRID_CONTACT_GROUPS
#ifdef GRID_GROUPED_TICK
  if(update_list[0]!=(unsigned)(uintptr_t)bodies[last_active]||update_list[1]!=(unsigned)(uintptr_t)world){error=10;break;}
  for(unsigned j=0;j<h[0];j++)if(get(bodies[j],0x318)!=(update_next[j]?(unsigned)(uintptr_t)bodies[update_next[j]-1]:0)){error=10;break;}
  if(error)break;
#endif
  memcpy(manager_before+0x74,manager+0x74,4);
  for(unsigned o=0xa8;o<=0xb8;o+=4)memcpy(manager_before+o,manager+o,4);
  const unsigned changed[]={0x38,0x3c,0x40,0x44,0x48,0x50,0x60,0x64,0x7f8,0x7fc};
  for(unsigned j=0;j<sizeof(changed)/sizeof(*changed);j++)memcpy(world_before+changed[j],world+changed[j],4);
#ifdef GRID_WHOLE_PHYSICAL_WORLD
  if(get(world,0x7c4)||get(world,0x7c8)!=*(unsigned *)parameters||get(world,0x7cc)||get(producer,0x924)||get(producer,0xa78)||get(producer,0xa7c)||get(producer,0x930)||get(producer,0xc20)||producer[0xc24]){error=10;break;}
  put(world_before,0x7c8,*(unsigned *)parameters);put(producer_before,0x924,0);put(producer_before,0xa78,0);put(producer_before,0xa7c,0);
  put(worker_before,0x1c,get(worker_before,0x18));put(worker_before,0x40,get(worker_before,0xc));
  if(memcmp(worker_before,worker,sizeof(worker))){error=10;break;}
  for(unsigned j=0;j<sizeof(broadphase);j++)if(broadphase[j])error=10;
  for(unsigned j=0;j<sizeof(worker_contacts);j++)if(worker_contacts[j]||worker_pairs[j])error=10;
  if(worker_ptrs[0]!=(unsigned)(uintptr_t)worker||get(world_wrapper,4)!=(unsigned)(uintptr_t)world){error=10;break;}
#endif
  if(memcmp(world_before,world,sizeof(world))||memcmp(producer_before,producer,sizeof(producer))){error=10;break;}
#endif
  if(memcmp(manager_before,manager,0x20)||memcmp(manager_before+0x5c,manager+0x5c,0x60)||memcmp(manager_before+0xcc,manager+0xcc,0x34)){error=10;break;}
  ends[0]=get(manager,0xc4);ends[1]=get(manager,0xbc);ends[2]=get(manager,0xc8);ends[3]=get(manager,0xc0);memcpy(cache,manager+0x20,60);
  for(unsigned j=0;j<2;j++){if(!cache[j])cache[j]=0xffffffff;else{unsigned found=0;for(unsigned m=0;m<h[1];m++)if(cache[j]==(unsigned)(uintptr_t)materials[m]){cache[j]=m;found=1;break;}if(!found)error=10;}}
#ifdef GRID_CONTACT_GROUPS
  const unsigned fields[11]={0x50,0x74,0x7fc,0x7f8,0x38,0x60,0x64,0x3c,0x40,0x44,0x48};
  for(unsigned j=0;j<11;j++)state[j]=get(j==1?manager:world,fields[j]);
  for(unsigned j=0;j<11;j++)if(j==0||j==1||j==4||j>=7)state[j]=owned_token(state[j],bodies[0],0x430,h[0],&error);
  unsigned last_root=0;
  for(unsigned root=get(manager,0x74),count=0;root;root=get((void *)(uintptr_t)root,0x1c)){
   if(++count>h[0]||!owned_token(root,bodies[0],0x430,h[0],&error)||get((void *)(uintptr_t)root,0x30)){error=10;break;}
   unsigned constructed=0,normals=0,contact=get((void *)(uintptr_t)root,0),n=0;
   for(;contact;contact=get((void *)(uintptr_t)contact,0x10)){
    if(++n>h[2]||!owned_token(contact,contacts[0],0x80,h[2],&error)){error=10;break;}
    constructed+=get((void *)(uintptr_t)contact,0x14);normals+=get((void *)(uintptr_t)contact,0x18);
   }
   if(constructed>h[3]||normals>constructed){error=10;break;}
   if(h[5])for(unsigned r=0;r<constructed;r++)ref_limits[r]=r>=normals?normals:0;
   last_root=root;
  }
  if(!last_root||get(manager,0xa8)!=get((void *)(uintptr_t)last_root,0x20)||get(manager,0xac)+get(manager,0xb4)!=get((void *)(uintptr_t)last_root,0x24)||get(manager,0xb0)||get(manager,0xb8)){error=10;break;}
  for(unsigned r=0;r<h[3];r++)if(ref_limits[r]){
   unsigned offset=get(rows[r],0xa8)-(unsigned)(uintptr_t)rows;
   if(offset%192!=0x98||offset/192>=ref_limits[r]){error=10;break;}put(rows[r],0xa8,offset/192);
  }
#else
  unsigned constructed=0;for(unsigned j=0;j<h[2];j++)constructed+=get(contacts[j],0x14);
  if(success==0&&get(manager,0x118)&&h[2])for(unsigned r=ends[1];r<constructed;r++){
   unsigned offset=get(rows[r],0xa8)-(unsigned)(uintptr_t)rows;
   if(offset%192!=0x98||offset/192>=ends[1]){error=10;break;}put(rows[r],0xa8,offset/192);
  }
#endif
  if(error)break;
  fwrite(&success,4,1,output);unsigned started=get(manager,0x118);fwrite(&started,4,1,output);fwrite(ends,4,4,output);fwrite(cache,4,15,output);
  for(unsigned j=0;j<h[0];j++){fwrite(mass+j,4,1,output);fwrite(inertia[j],4,16,output);}fwrite(workspace,32,h[0],output);
  for(unsigned j=0;j<h[0];j++){
#ifdef GRID_CONTACT_GROUPS
   if(get(bodies[j],0x32c)!=(unsigned)(uintptr_t)world||get(bodies[j],0x334)!=(unsigned)(uintptr_t)(bodies[j]+0x1c0))error=10;
   for(unsigned k=0;k<3;k++)if(get(bodies[j],0x338+k*4)!=(unsigned)(uintptr_t)(bodies[j]+0x190+k*16))error=10;
#endif
   put(bodies[j],0x334,0x1c0);put(bodies[j],0x32c,0);
#ifdef GRID_CONTACT_GROUPS
   const unsigned links[]={8,0xc,0x10,0x14,0x18,0x1c,0x2f0,0x2f4,0x2fc,0x300,0x304};
   for(unsigned k=0;k<sizeof(links)/sizeof(*links);k++)put(bodies[j],links[k],owned_token(get(bodies[j],links[k]),bodies[0],0x430,h[0],&error));
   for(unsigned k=0;k<2;k++)put(bodies[j],k*4,owned_token(get(bodies[j],k*4),contacts[0],0x80,h[2],&error));
#ifdef GRID_GROUPED_TICK
   put(bodies[j],0x318,update_next[j]);
#endif
#else
   put(bodies[j],0x2fc,0);
#endif
   for(unsigned k=0;k<3;k++)put(bodies[j],0x338+4*k,0);fwrite(bodies[j],1,0x424,output);}
  for(unsigned j=0;j<h[2];j++){unsigned char *c=contacts[j];
#ifdef GRID_CONTACT_GROUPS
   if(get(c,4)!=(unsigned)(uintptr_t)bodies[body_ids[j][0]]||get(c,8)!=(modes[j]==2?0:(unsigned)(uintptr_t)bodies[body_ids[j][1]])||get(c,0x30)!=(unsigned)(uintptr_t)materials[material_ids[j][0]]||get(c,0x34)!=(unsigned)(uintptr_t)materials[material_ids[j][1]])error=10;
   if(get(c,0x28)!=(shape_ids[j][1]?(unsigned)(uintptr_t)shapes[shape_ids[j][1]-1]:0)||get(c,0x2c)!=(shape_ids[j][0]?(unsigned)(uintptr_t)shapes[shape_ids[j][0]-1]:0))error=10;
   put(c,0x10,owned_token(get(c,0x10),contacts[0],0x80,h[2],&error));put(c,4,body_ids[j][0]);put(c,8,body_ids[j][1]);
#else
   put(c,0x10,0);put(c,4,get(c,0x1c));put(c,8,get(c,0x20));
#endif
   put(c,0x30,material_ids[j][0]);put(c,0x34,material_ids[j][1]);put(c,0x2c,shape_ids[j][0]);put(c,0x28,shape_ids[j][1]);fwrite(c,1,128,output);}
#ifdef GRID_CONTACT_GROUPS
  if(error)break;
#endif
#ifdef GRID_OWNED_SHAPES
  for(unsigned s=0;s<shape_count;s++)for(unsigned o=0;o<0x240;o++){
   unsigned char expected=o==0x216?shape_flags[s][0]:o==0x217?shape_flags[s][1]:o==0x229?shape_flags[s][2]:o==0x230?shape_flags[s][3]:0;
   if(shapes[s][o]!=expected)error=10;
  }
  if(error)break;
#endif
  fwrite(rows,192,h[3],output);++cases;
#ifdef GRID_CONTACT_GROUPS
  fwrite(state,4,11,output);
#ifdef GRID_WHOLE_PHYSICAL_WORLD
  unsigned lifecycle[6]={get(world,0x7c4),get(world,0x7c8),get(producer,0x924),get(producer,0xa78),get(producer,0xa7c),get(world,0x7cc)};fwrite(lifecycle,4,6,output);
#endif
#endif
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;
 if(input&&fclose(input))error=14;if(output&&fclose(output))error=14;
 #if defined(GRID_WORLD_ADVANCE) || defined(GRID_CONTACT_GROUPS)
 if(error&&output)_wremove(argv[3]);
#endif
 free(code);VirtualFree(image,0,MEM_RELEASE);
 printf("physical_tick_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);
#ifdef GRID_WHOLE_PHYSICAL_WORLD
 printf("whole_serial_world=1 wrapper_pool_consumed=1 owned_workers_retained=1\n");
#endif
 return (int)error;
}
