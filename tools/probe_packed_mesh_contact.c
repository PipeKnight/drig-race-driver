/* Whole original B39500 -> mesh7/packed11 contact dispatch with owned caches.
   PC01 carries a verified CM05 seed and a bounded source packed chunk. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <math.h>
#include <float.h>
#include "isolated_reference.h"
#include "owned_collision_mesh_probe.h"
#include "owned_packed_shape_probe.h"
#ifdef GRID_PACKED_CONTACT_SCHEDULE
#define ROOT_BYTES 0xc40
#define REQUEST_TAG 0x32304350
#else
#define ROOT_BYTES 0xa80
#define REQUEST_TAG 0x31304350
#endif
typedef struct {unsigned char before[32]; __declspec(align(16)) unsigned char value[0x2050]; unsigned char after[32];} Cache;
static int canary(const unsigned char *p,unsigned size){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+size+i]!=0xcd)return 0;return 1;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;original=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot)return 5;memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) unsigned char seed[22000],mesh[16000],mesh_before[16000],first_owned[32+0x250+32],root_owned[32+ROOT_BYTES+32],pair_owned[32+0x50+32],body[0x430],body_before[0x430],materials[0x40];
 __declspec(align(16)) unsigned char worker_owned[32+0x48+32],contacts_owned[32+16*0x80+32],scratch_owned[32+0x570+32],triangle_owned[32+0x280+32];
 __declspec(align(16)) Cache cache;
 struct {unsigned char before[32];Object object;unsigned char after[32];} track_owned;
 Object *track_pointer=&track_owned.object;
#define track (*track_pointer)
 unsigned char *first=first_owned+32,*contacts=contacts_owned+32,*scratch=scratch_owned+32,*triangle=triangle_owned+32,*root=root_owned+32,*pair=pair_owned+32,*worker=worker_owned+32;
 unsigned char *chunk=NULL,*chunk_before=NULL,*indices_owned=NULL,*visited_owned=NULL,*indices=NULL,*visited=NULL;
 unsigned error=0,protected=0,header[6],mesh_bytes=0,seed_size=0,chunk_size=0,limit=0,cache_limit=0,index_limit=0;float center[4];FILE *input=NULL;
#ifdef GRID_PACKED_CONTACT_SCHEDULE
 unsigned schedule_flags=0,worker_pointer=address(worker),group_head=0,assigned_head=0;
#endif
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xf5e000};DWORD protections[5];unsigned char saved[5][4096];
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(header,4,6,input)!=6||header[0]!=REQUEST_TAG||header[1]<0x1680||header[1]>sizeof(seed)||header[2]<52||header[2]>2*1024*1024||header[3]>16||header[4]>1||header[5]>32768||fread(center,4,4,input)!=4){error=7;goto done;}
#ifdef GRID_PACKED_CONTACT_SCHEDULE
 if(fread(&schedule_flags,4,1,input)!=1||schedule_flags>3){error=7;goto done;}
#endif
 seed_size=header[1];chunk_size=header[2];limit=header[3];cache_limit=header[4];index_limit=header[5];
 for(unsigned i=0;i<4;i++)if(!isfinite(center[i])||fabsf(center[i])>10000.f||(i==3&&center[i]!=1.f)){error=8;goto done;}
 chunk=malloc(chunk_size);chunk_before=malloc(chunk_size);indices_owned=malloc(64+index_limit*4);visited_owned=malloc(64+index_limit*4);
 if(!chunk||!chunk_before||!indices_owned||!visited_owned){error=9;goto done;}indices=indices_owned+32;visited=visited_owned+32;
 if(fread(seed,1,seed_size,input)!=seed_size||fread(chunk,1,chunk_size,input)!=chunk_size||fgetc(input)!=EOF){error=7;goto done;}
 mesh_bytes=seed_size-0x1620;
 if(mesh_bytes<0x60||mesh_bytes>16000||get(seed,0x10)!=7||get(seed,0x240)!=0x1500||get(seed,0x188)||get(seed,0x184)||get(seed,0xdc)||get(seed,0x500+mesh_bytes+0x320)){error=8;goto done;}
 unsigned points=get(seed+0x500,0),faces=get(seed+0x500,4),face_start=0x60+points*48;
 if(points<4||points>80||faces<4||faces>110||face_start+faces*32>mesh_bytes||get(seed+0x500,0x54)!=0x60||get(seed+0x500,0x58)!=face_start){error=8;goto done;}
 for(unsigned i=0;i<points;i++)for(unsigned j=0;j<4;j++)if(!isfinite(num(seed+0x500,0x60+i*32+j*4))||fabsf(num(seed+0x500,0x60+i*32+j*4))>100000.f||(j==3&&num(seed+0x500,0x60+i*32+j*4)!=1.f)){error=8;goto done;}
 for(unsigned i=0;i<faces;i++)for(unsigned j=0;j<3;j++)if(get(seed+0x500,face_start+i*32+16+j*4)>=points){error=8;goto done;}
 for(unsigned j=0;j<16;j++)if(!isfinite(num(seed,0x130+j*4))||fabsf(num(seed,0x130+j*4))>10000.f){error=8;goto done;}
 for(unsigned j=0;j<4;j++)if(!isfinite(num(seed,0x500+mesh_bytes+0x1c0+j*4))){error=8;goto done;}
 memset(&track_owned,0xcd,sizeof(track_owned));
 if(!isfinite(num(seed,0x220))||num(seed,0x220)<0.f||num(seed,0x220)>1e6f||!prepare(chunk,chunk_size,&track)){error=8;goto done;}
 memcpy(chunk_before,chunk,chunk_size);memset(first_owned,0xcd,sizeof(first_owned));memcpy(first,seed,0x250);memcpy(mesh,seed+0x500,mesh_bytes);memcpy(mesh_before,mesh,mesh_bytes);memcpy(body,seed+0x500+mesh_bytes,sizeof(body));
 if(!relocate_mesh(mesh,mesh_bytes,1)){error=8;goto done;}
 memset(root_owned,0xcd,sizeof(root_owned));memset(pair_owned,0xcd,sizeof(pair_owned));memset(worker_owned,0xcd,sizeof(worker_owned));memset(root,0,ROOT_BYTES);memset(pair,0,0x50);memset(worker,0,0x48);memset(materials,0,sizeof(materials));memset(&cache,0xcd,sizeof(cache));memset(cache.value,0,sizeof(cache.value));memset(indices_owned,0xcd,64+index_limit*4);memset(visited_owned,0xcd,64+index_limit*4);memset(indices,0,index_limit*4);memset(visited,0,index_limit*4);
 memset(contacts_owned,0xcd,sizeof(contacts_owned));memset(contacts,0,16*0x80);memset(scratch_owned,0xcd,sizeof(scratch_owned));memset(scratch,0,0x570);memset(triangle_owned,0xcd,sizeof(triangle_owned));memset(triangle,0,0x280);
 put(first,0,address(image+0xd3e7e0-0x400000));put(first,0xcc,address(root));put(first,0xc8,address(materials));put(first,0xd8,address(body));put(first,0x240,address(mesh));put(first,0x21c,0);put(first,0x224,0x7f7fffff);
 for(unsigned j=0;j<4;j++)put(first,0xb0+j*4,address(first+0x70+j*16));
 for(unsigned j=0;j<5;j++)put(first,0x170+j*4,address(first+0x130+(j>0?(j-1)*16:0)));
 for(unsigned j=0;j<3;j++){unsigned a=0x19c+j*40,b=a+20;put(first,a,address(first+b));put(first,a+4,0);put(first,a+8,address(first));put(first,b,0);put(first,b+4,address(first+a));put(first,b+8,address(first));}
 for(unsigned j=0;j<3;j++){float delta=center[j]-num(body,0x1c0+j*4),v=num(first,0x160+j*4)+delta;memcpy(first+0x160+j*4,&v,4);memcpy(body+0x1c0+j*4,center+j,4);memcpy(body+0x2a0+j*4,center+j,4);}
 put(body,0x334,address(body+0x1c0));memcpy(body_before,body,sizeof(body));
 for(unsigned o=0xd0;o<=0xec;o+=4)if(o!=0xd8&&o!=0xdc)put(first,o,0);
 first[0x15]=track.shape[0x15]=1;first[0x14]=track.shape[0x14]=0;first[0x1a]=track.shape[0x1a]=0xff;
 put(track.shape,0xcc,address(root));put(track.shape,0xc8,address(materials));put(track.shape,0x224,0x7f7fffff);
 for(unsigned i=0;i<get(track.header,0x20);i++)put(track.header,0x34+i*4,address(materials));
 put(pair,0x28,address(first));put(pair,0x2c,address(track.shape));
 put(root,0x5f0,address(image+0xaea1f0-0x400000));put(root,0x5f4,address(image+0xade540-0x400000));put(root,0x5f8,address(image+0xae4700-0x400000));
 put(worker,0,address(scratch));put(worker,4,limit);put(worker,0xc,address(contacts));put(worker,0x14,limit);put(worker,0x34,cache_limit);put(worker,0x38,address(cache.value));put(worker,0x40,address(contacts));
 put(cache.value,0x20,address(indices));put(cache.value,0x24,address(visited));put(cache.value,0x28,index_limit);put(cache.value,0x38,address(triangle));put(cache.value,0x3c,address(triangle));
#ifdef GRID_PACKED_CONTACT_SCHEDULE
 put(root,0x8b0,address(image+0xaea1f0-0x400000));put(root,0x8b4,address(image+0xade540-0x400000));put(root,0x8b8,address(image+0xae4700-0x400000));put(root,0x8bc,1);
 put(root,0x928,address(&worker_pointer));put(root,0x92c,1);put(body,0x328,address(first));body[0x416]=(unsigned char)(schedule_flags&1);put(body,0x300,0);put(body,0x400,0);
 if(schedule_flags&2){put(pair,0x28,address(track.shape));put(pair,0x2c,address(first));put(first,0xd4,address(pair));put(track.shape,0xd0,address(pair));}
 else {put(first,0xd0,address(pair));put(track.shape,0xd4,address(pair));}
 memcpy(body_before,body,sizeof(body));
#endif
 for(unsigned i=0;i<5;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,protections+i)){error=10;goto done;}++protected;}
 __try{
  ((void (__cdecl *)(void))(image+0xc7dae0-0x400000))();
  for(unsigned i=0;i<16;i++)((void (__fastcall *)(void *,void *))(image+0xb0ece0-0x400000))(contacts+i*0x80,NULL);
  ((void (__fastcall *)(void *,void *))(image+0xb7e440-0x400000))(scratch,NULL);
  ((void (__fastcall *)(void *,void *))(image+0xb7c5a0-0x400000))(triangle,NULL);put(triangle,0x220,0);
  ((void (__fastcall *)(void *,void *))(image+0xabc7b0-0x400000))(first,NULL);
  ((void (__cdecl *)(void *,void *,void *))(image+0xb42050-0x400000))(first,first+0x130,first+0x20);
#ifdef GRID_PACKED_CONTACT_SCHEDULE
  ((void (__fastcall *)(void *,void *))(image+0xb152e0-0x400000))(root,NULL);
  ((void (__fastcall *)(void *,void *,void *))(image+0xb18300-0x400000))(root,NULL,body);
  assigned_head=get(worker,8);
  ((void (__fastcall *)(void *,void *,void *))(image+0xb184f0-0x400000))(root,NULL,&group_head);
  ((void (__fastcall *)(void *,void *))(image+0xb187d0-0x400000))(root,NULL);
  if(group_head||root[0xc24]||get(root,0xa78)!=(schedule_flags&1)||get(worker,8)||assigned_head!=((schedule_flags&1)?address(pair):0)||get(root,0x930)!=((schedule_flags&1)?address(pair):0)){error=20;}
#else
  ((unsigned (__fastcall *)(void *,void *,void *,void *))(image+0xb39500-0x400000))(worker,NULL,root,pair);
#endif
 }__except(fault(GetExceptionInformation())){error=11;}
 if(!relocate_mesh(mesh,mesh_bytes,0)||memcmp(mesh,mesh_before,mesh_bytes)||memcmp(chunk,chunk_before,chunk_size)||memcmp(body,body_before,sizeof(body)))error=12;
 if(!error){
  Region regions[]={{first,0x250,0x1000},{track.shape,0x280,0xc000},{root,ROOT_BYTES,0x20000},{pair,0x50,0xd000},{body,0x430,0xa000},{mesh,mesh_bytes,0x30000},{materials,0x40,0x1420},{contacts,16*0x80+1,0xe000},{scratch,0x570,0xf000},{cache.value,0x2050,0x40000},{triangle,0x280,0x50000},{track.header,0x60,0x60000},{chunk,chunk_size,0x100000},{indices,index_limit*4+1,0x400000},{visited,index_limit*4+1,0x500000}
#ifdef GRID_PACKED_CONTACT_SCHEDULE
  ,{(unsigned char *)&worker_pointer,4,0x70000},{worker,0x48,0x80000}
#endif
  };
  unsigned n=sizeof(regions)/sizeof(regions[0]);
  const unsigned pointers[]={0xb0,0xb4,0xb8,0xbc,0xc8,0xcc,0xd0,0xd4,0xd8,0xe0,0xe4,0xe8,0xec,0x170,0x174,0x178,0x17c,0x180,0x19c,0x1a0,0x1a4,0x1b0,0x1b4,0x1b8,0x1c4,0x1c8,0x1cc,0x1d8,0x1dc,0x1e0,0x1ec,0x1f0,0x1f4,0x200,0x204,0x208,0x21c};
  unsigned char *shapes[]={first,track.shape,triangle};for(unsigned s=0;s<3;s++)for(unsigned j=0;j<sizeof(pointers)/sizeof(pointers[0]);j++)if(!canonical(shapes[s],pointers[j],regions,n)){fprintf(stderr,"pointer shape=%u offset=%x value=%08x\n",s,pointers[j],get(shapes[s],pointers[j]));error=13;}
  if(get(first,0)!=address(image+0xd3e7e0-0x400000)||get(triangle,0)!=address(image+0xd42670-0x400000))error=13;put(first,0,0x7000);put(triangle,0,0x7300);
  if(!canonical(first,0x240,regions,n)||!canonical(track.shape,0x244,regions,n)||!canonical(triangle,0x270,regions,n))error=13;
  for(unsigned o=0x24;o<=0x30;o+=4)if(!canonical(track.header,o,regions,n))error=13;
  for(unsigned i=0;i<get(track.header,0x20);i++)if(!canonical(track.header,0x34+i*4,regions,n))error=13;
  const unsigned wp[]={0,0xc,0x38,0x40};for(unsigned i=0;i<4;i++)if(!canonical(worker,wp[i],regions,n))error=13;
  const unsigned cp[]={0x20,0x24,0x38,0x3c,0x40};for(unsigned i=0;i<5;i++)if(!canonical(cache.value,cp[i],regions,n))error=13;
  unsigned count=get(worker,0x10),candidates=get(cache.value,0x2c),seen=get(cache.value,0x30);
  if(count>limit||get(pair,0x24)!=count||get(worker,0x40)!=0xe000+count*0x80||candidates>index_limit||seen>index_limit||get(worker,0x30)>cache_limit)error=14;
  for(unsigned i=0;i<candidates;i++)if(!canonical(indices,i*4,regions,n))error=13;
  for(unsigned i=0;i<seen;i++)if(!canonical(visited,i*4,regions,n))error=13;
  const unsigned pp[]={0x20,0x28,0x2c};for(unsigned i=0;i<3;i++)if(!canonical(pair,pp[i],regions,n))error=13;
  for(unsigned i=0;i<16;i++){unsigned char *c=contacts+i*0x80;if(get(c,0)!=address(image+0xd3d434-0x400000))error=13;put(c,0,0x7200);if(i<count){const unsigned offsets[]={4,8,0x28,0x2c,0x30,0x34};for(unsigned j=0;j<6;j++)if(!canonical(c,offsets[j],regions,n))error=13;for(unsigned j=0x38;j<0x70;j+=4)if(!isfinite(num(c,j)))error=15;}}
  put(root,0x5f0,0x7100);put(root,0x5f4,0x7101);put(root,0x5f8,0x7102);put(body,0x334,0x1c0);
#ifdef GRID_PACKED_CONTACT_SCHEDULE
  put(root,0x8b0,0x7100);put(root,0x8b4,0x7101);put(root,0x8b8,0x7102);
  if(!canonical(root,0x928,regions,n)||!canonical(root,0x930,regions,n)||!canonical(root,0xc20,regions,n)||!canonical(worker,8,regions,n)||!canonical(body,0x328,regions,n)||!canonical(&worker_pointer,0,regions,n)||!canonical(&assigned_head,0,regions,n))error=13;
#endif
 }
 if(!canary(first_owned,0x250)||!canary(contacts_owned,16*0x80)||!canary(scratch_owned,0x570)||!canary(triangle_owned,0x280)||!canary((unsigned char *)&cache,0x2050)||!canary(root_owned,ROOT_BYTES)||!canary(pair_owned,0x50)||!canary(worker_owned,0x48)||!canary((unsigned char *)&track_owned,sizeof(Object))||(indices_owned&&!canary(indices_owned,index_limit*4))||(visited_owned&&!canary(visited_owned,index_limit*4)))error=16;
done:
 if(input)fclose(input);
 for(unsigned i=0;i<protected;i++){DWORD ignored;unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);if(!VirtualProtect(page,4096,protections[i],&ignored))error=17;}
 unsigned unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,(sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData)))unchanged=0;if(!unchanged)error=17;
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=18;else{const void *parts[]={first,track.shape,root,pair,worker,contacts,body,materials,scratch,cache.value,triangle,track.header,indices,visited
#ifdef GRID_PACKED_CONTACT_SCHEDULE
  ,root+0xa80,&worker_pointer,&group_head,&assigned_head
#endif
 };const DWORD lengths[]={0x250,0x280,0xa80,0x50,0x48,16*0x80,0x430,0x40,0x570,0x2050,0x280,0x60,index_limit*4,index_limit*4
#ifdef GRID_PACKED_CONTACT_SCHEDULE
  ,ROOT_BYTES-0xa80,4,4,4
#endif
 };for(unsigned i=0;i<sizeof(lengths)/sizeof(lengths[0]);i++){DWORD written;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=19;}CloseHandle(output);}}
 free(chunk);free(chunk_before);free(indices_owned);free(visited_owned);free(snapshot);VirtualFree(image,0,MEM_RELEASE);printf("packed_mesh_contact code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",unchanged,unchanged,error);return (int)error;
}
