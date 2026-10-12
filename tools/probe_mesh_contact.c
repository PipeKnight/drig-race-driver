/* Whole original mesh registration and B39500 mesh-to-mesh contact generation.
   MC02 seed is a separately verified CM05 result; no process attach or code edits. */
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
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static float num(const void *p,unsigned o){float v;memcpy(&v,(const char *)p+o,4);return v;}
static unsigned address(const void *p){return (unsigned)(uintptr_t)p;}
static uintptr_t original;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"mesh_contact_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-original+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
typedef struct {unsigned char *p;unsigned length,token;} Region;
static int canonical(void *p,unsigned o,const Region *regions,unsigned n){
 unsigned value=get(p,o);if(!value)return 1;
 for(unsigned i=0;i<n;i++)if(value>=address(regions[i].p)&&value-address(regions[i].p)<regions[i].length){put(p,o,regions[i].token+value-address(regions[i].p));return 1;}
 return 0;
}
static int mesh_pointer(unsigned char *mesh,unsigned offset,unsigned length,int native){
 unsigned v=get(mesh,offset),base=address(mesh);
 if(native){if(v>=length||(v&3))return 0;put(mesh,offset,base+v);}
 else {if(v<base||v-base>=length||((v-base)&3))return 0;put(mesh,offset,v-base);}
 return 1;
}
static int relocate_mesh(unsigned char *mesh,unsigned length,int native){
 unsigned points=get(mesh,0),faces=get(mesh,4),face=0x60+points*48;
 for(unsigned i=0;i<points;i++){
  unsigned vertex=0x60+i*32,aux=0x60+points*32+i*16;
  unsigned adjacent=get(mesh,aux+4)-(native?0:address(mesh)),n=get(mesh,aux);
  unsigned incident=get(mesh,vertex+0x18)-(native?0:address(mesh)),t=get(mesh,vertex+0x14);
  if(n>points||t>faces||adjacent>length||n*4>length-adjacent||incident>length||t*4>length-incident)return 0;
  for(unsigned j=0;j<n;j++){
   unsigned v=get(mesh,adjacent+j*4)-(native?0:address(mesh));
   if(v<0x60||v>=0x60+points*32||(v-0x60)%32||!mesh_pointer(mesh,adjacent+j*4,length,native))return 0;
  }
  for(unsigned j=0;j<t;j++){
   unsigned v=get(mesh,incident+j*4)-(native?0:address(mesh));
   if(v<face||v>=face+faces*32||(v-face)%32||!mesh_pointer(mesh,incident+j*4,length,native))return 0;
  }
  if(!mesh_pointer(mesh,vertex+0x10,length,native)||!mesh_pointer(mesh,vertex+0x18,length,native)||!mesh_pointer(mesh,aux+4,length,native))return 0;
 }
 return mesh_pointer(mesh,0x54,length,native)&&mesh_pointer(mesh,0x58,length,native);
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;original=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot)return 5;memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) unsigned char seed[22000],first_owned[32+0x250+32],second_owned[32+0x250+32],root_owned[32+0xa80+32],pair_owned[32+0x50+32],cells_owned[32+16+32],body[0x430],manager[0x20],materials[0x40],mesh[16000],mesh_before[16000];
 unsigned char *first=first_owned+32,*second=second_owned+32,*root=root_owned+32,*pair=pair_owned+32,*cells=cells_owned+32;
 __declspec(align(16)) unsigned char worker_owned[32+0x48+32],contacts_owned[32+16*0x80+32],body_before[0x430],scratch_owned[32+0x570+32];
 unsigned char *worker=worker_owned+32,*contacts=contacts_owned+32,*scratch=scratch_owned+32;
 memset(scratch_owned,0xcd,sizeof(scratch_owned));memset(scratch,0,0x570);memset(worker_owned,0xcd,sizeof(worker_owned));memset(contacts_owned,0xcd,sizeof(contacts_owned));memset(worker,0,0x48);memset(contacts,0,16*0x80);
 unsigned error=0,protected=0,mesh_bytes=0,tag=0,mask=0,capacity=0,seed_size=0,contact_capacity=0;float shift[4],contact_limit=0;FILE *input=NULL;
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xf5e000};DWORD protections[5];unsigned char saved[5][4096];
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(&tag,4,1,input)!=1||tag!=0x3230434d||fread(&mesh_bytes,4,1,input)!=1||mesh_bytes<0x60||mesh_bytes>16000||fread(shift,4,4,input)!=4||fread(&mask,4,1,input)!=1||fread(&capacity,4,1,input)!=1||capacity>1||fread(&contact_limit,4,1,input)!=1||!isfinite(contact_limit)||contact_limit<0.f||fread(&contact_capacity,4,1,input)!=1||contact_capacity>16){error=7;goto done;}
 seed_size=0x500+mesh_bytes+0x1120;
 if(seed_size>sizeof(seed)||fread(seed,1,seed_size,input)!=seed_size||fgetc(input)!=EOF){error=7;goto done;}
 if(get(seed,0x188)||get(seed,0xdc)||get(seed,0x500+mesh_bytes+0x320)){error=8;goto done;}
 for(unsigned j=0;j<4;j++)if(!isfinite(shift[j])||fabsf(shift[j])>10000.f||(j==3&&shift[j]!=0.f)){error=8;goto done;}
 unsigned points=get(seed+0x500,0),faces=get(seed+0x500,4),face_start=0x60+points*48;
 if(points<4||points>80||faces<4||faces>110||face_start+faces*32>mesh_bytes||get(seed+0x500,0x54)!=0x60||get(seed+0x500,0x58)!=face_start||get(seed,0x10)!=7||get(seed,0x240)!=0x1500||seed[0x19]>31||get(seed,0x198)!=1){error=8;goto done;}
 for(unsigned i=0;i<points;i++)for(unsigned j=0;j<4;j++)if(!isfinite(num(seed+0x500,0x60+i*32+j*4))||fabsf(num(seed+0x500,0x60+i*32+j*4))>100000.f||(j==3&&num(seed+0x500,0x60+i*32+j*4)!=1.f)){error=8;goto done;}
 for(unsigned i=0;i<faces;i++)for(unsigned j=0;j<3;j++)if(get(seed+0x500,face_start+i*32+16+j*4)>=points){error=8;goto done;}
 for(unsigned j=0;j<16;j++)if(!isfinite(num(seed,0x130+j*4))||fabsf(num(seed,0x130+j*4))>10000.f){error=8;goto done;}
 if(!isfinite(num(seed,0x220))||num(seed,0x220)<0.f||num(seed,0x220)>1e6f||get(seed,0x194)>=1000000){error=8;goto done;}
 memset(first_owned,0xcd,sizeof(first_owned));memset(second_owned,0xcd,sizeof(second_owned));memset(root_owned,0xcd,sizeof(root_owned));memset(pair_owned,0xcd,sizeof(pair_owned));memset(cells_owned,0xcd,sizeof(cells_owned));
 memcpy(first,seed,0x250);memcpy(mesh,seed+0x500,mesh_bytes);memcpy(mesh_before,mesh,mesh_bytes);memset(second,0xcd,0x250);memset(root,0,0xa80);memset(pair,0,0x50);memset(cells,0,16);memcpy(body,seed+0x500+mesh_bytes,sizeof(body));put(body,0x334,address(body+0x1c0));memcpy(body_before,body,sizeof(body));memset(manager,0,sizeof(manager));memset(materials,0,sizeof(materials));
 if(!relocate_mesh(mesh,mesh_bytes,1)){error=17;goto done;}
 put(first,0,address(image+0xd3e7e0-0x400000));put(first,0xcc,address(root));put(first,0xc8,0);put(first,0xd8,address(body));put(first,0x240,address(mesh));put(first,0x21c,0);memcpy(first+0x224,&contact_limit,4);
 for(unsigned j=0;j<4;j++)put(first,0xb0+j*4,address(first+0x70+j*16));
 for(unsigned j=0;j<5;j++)put(first,0x170+j*4,address(first+0x130+(j>0?(j-1)*16:0)));
 for(unsigned j=0;j<3;j++){
  unsigned a=0x19c+j*40,b=a+20;put(first,a,address(first+b));put(first,a+4,0);put(first,a+8,address(first));put(first,b,0);put(first,b+4,address(first+a));put(first,b+8,address(first));put(root,0x900+j*4,address(first+a));
 }
 for(unsigned o=0xd0;o<=0xd4;o+=4)put(first,o,0);
 for(unsigned o=0xe0;o<=0xec;o+=4)put(first,o,0);
 put(root,0x14,address(manager));put(manager,4,address(materials));put(manager,8,2);put(manager,0xc,0x20);
 put(root,0x80,get(first,0x194)+1);put(root,0x940,0x447a0000);put(root,0x944,0x447a0000);put(root,0x948,1);put(root,0x94c,1);put(root,0x950,address(cells));put(root,0x954,address(cells+8));put(cells,12,address(first));put(root,0x914,capacity?address(pair):0);
 for(unsigned j=0;j<32;j++)put(root,0x974+j*4,mask);
 for(unsigned i=0;i<5;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,protections+i)){error=9;goto done;}++protected;}
 __declspec(align(16)) unsigned descriptor[33];memcpy(descriptor,seed+0x250,sizeof(descriptor));descriptor[0]=address(mesh);descriptor[0x5c/4]=address(root);((unsigned char *)descriptor)[0x50]=0;memcpy((unsigned char *)descriptor+0x6c,&contact_limit,4);
 __try{
  ((void (__cdecl *)(void))(image+0xc7dae0-0x400000))();
  for(unsigned i=0;i<16;i++)((void (__fastcall *)(void *,void *))(image+0xb0ece0-0x400000))(contacts+i*0x80,NULL);
  ((void (__fastcall *)(void *,void *,void *))(image+0xb460f0-0x400000))(second,NULL,descriptor);
  memcpy(second+0x130,first+0x130,64);for(unsigned j=0;j<3;j++){float v=num(second,0x160+j*4)+shift[j];memcpy(second+0x160+j*4,&v,4);}
  ((void (__fastcall *)(void *,void *))(image+0xabc7b0-0x400000))(second,NULL);
  ((void (__fastcall *)(void *,void *,void *))(image+0xb16d70-0x400000))(root,NULL,second);
  first[0x15]=second[0x15]=1;first[0x14]=second[0x14]=0;first[0x1a]=second[0x1a]=0xff;
  put(first,0xc8,address(materials));put(second,0xc8,address(materials));
  put(root,0x5b0,address(image+0xae8ad0-0x400000));put(root,0x5b4,address(image+0xadbd50-0x400000));put(root,0x5b8,address(image+0xae34b0-0x400000));
  ((void (__fastcall *)(void *,void *))(image+0xb7e440-0x400000))(scratch,NULL);
  put(worker,0,address(scratch));put(worker,4,contact_capacity);put(worker,0x14,contact_capacity);put(worker,0x40,address(contacts));
  if(get(root,0x910))((unsigned (__fastcall *)(void *,void *,void *,void *))(image+0xb39500-0x400000))(worker,NULL,root,pair);
 }__except(fault(GetExceptionInformation())){error=10;}
 if(!relocate_mesh(mesh,mesh_bytes,0)||memcmp(mesh,mesh_before,mesh_bytes))error=11;
 if(!error){
  Region regions[]={{first,0x250,0x1000},{second,0x250,0xc000},{root,0xa80,0x12e0},{pair,0x50,0xd000},{cells,16,0xb000},{body,0x430,0xa000},{mesh,mesh_bytes,0x1500},{manager,0x20,0x1400},{materials,0x40,0x1420},{contacts,16*0x80+1,0xe000},{scratch,0x570,0xf000}};
  const unsigned pointers[]={0xb0,0xb4,0xb8,0xbc,0xc8,0xcc,0xd0,0xd4,0xd8,0xe0,0xe4,0xe8,0xec,0x170,0x174,0x178,0x17c,0x180,0x19c,0x1a0,0x1a4,0x1b0,0x1b4,0x1b8,0x1c4,0x1c8,0x1cc,0x1d8,0x1dc,0x1e0,0x1ec,0x1f0,0x1f4,0x200,0x204,0x208,0x21c,0x240};
  for(unsigned s=0;s<2;s++){unsigned char *shape=s?second:first;put(shape,0,0x7000);for(unsigned j=0;j<sizeof(pointers)/sizeof(pointers[0]);j++)if(!canonical(shape,pointers[j],regions,11))error=12;}
  const unsigned world_pointers[]={0x14,0x900,0x904,0x908,0x910,0x914,0x950,0x954};
  for(unsigned j=0;j<sizeof(world_pointers)/sizeof(world_pointers[0]);j++)if(!canonical(root,world_pointers[j],regions,11))error=12;
  for(unsigned j=0;j<16;j+=4)if(!canonical(cells,j,regions,11))error=12;
  const unsigned pair_pointers[]={4,8,0x20,0x28,0x2c,0x30,0x34,0x38,0x3c};
  for(unsigned j=0;j<sizeof(pair_pointers)/sizeof(pair_pointers[0]);j++)if(!canonical(pair,pair_pointers[j],regions,11))error=12;
  if(memcmp(body,body_before,sizeof(body)))error=13;
  unsigned count=get(worker,0x10);if(count>16||get(pair,0x24)!=count||get(worker,0x40)!=address(contacts)+count*0x80)error=18;
  if(!canonical(worker,0,regions,11)||!canonical(worker,0x40,regions,11))error=12;
  for(unsigned i=0;i<16;i++){if(get(contacts+i*0x80,0)!=address(image+0xd3d434-0x400000))error=12;put(contacts+i*0x80,0,0x7200);}
  for(unsigned i=0;i<count;i++){
   unsigned char *contact=contacts+i*0x80;const unsigned offsets[]={4,8,0x28,0x2c,0x30,0x34};
   for(unsigned j=0;j<6;j++)if(!canonical(contact,offsets[j],regions,11))error=12;
   for(unsigned j=0x38;j<0x70;j+=4)if(!isfinite(num(contact,j))){fprintf(stderr,"mesh_contact_nonfinite record=%u offset=%x bits=%08x\n",i,j,get(contact,j));error=19;}
  }
  for(unsigned j=0;j<32;j++)if(worker_owned[j]!=0xcd||worker_owned[32+0x48+j]!=0xcd||contacts_owned[j]!=0xcd||contacts_owned[32+16*0x80+j]!=0xcd||scratch_owned[j]!=0xcd||scratch_owned[32+0x570+j]!=0xcd)error=13;
  put(root,0x5b0,0x7100);put(root,0x5b4,0x7101);put(root,0x5b8,0x7102);
  put(body,0x334,0x1c0);
  unsigned char *owners[]={first_owned,second_owned,root_owned,pair_owned,cells_owned};const unsigned sizes[]={0x250,0x250,0xa80,0x50,16};
  for(unsigned j=0;j<5;j++)for(unsigned i=0;i<32;i++)if(owners[j][i]!=0xcd||owners[j][32+sizes[j]+i]!=0xcd)error=13;
 }
done:
 if(input)fclose(input);
 for(unsigned i=0;i<protected;i++){DWORD ignored;unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);if(!VirtualProtect(page,4096,protections[i],&ignored))error=14;}
 unsigned unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,(sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData)))unchanged=0;
 if(!unchanged)error=14;
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=15;else{const void *parts[]={first,second,root,cells,pair,worker,contacts,body,materials,scratch};const DWORD lengths[]={0x250,0x250,0xa80,16,0x50,0x48,16*0x80,0x430,0x40,0x570};for(unsigned i=0;i<10;i++){DWORD written;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=16;}CloseHandle(output);}}
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);printf("mesh_contact code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",unchanged,unchanged,error);return (int)error;
}
