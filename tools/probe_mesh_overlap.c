/* Whole original second mesh registration against an owned F217 first shape.
   MP01 seed is a separately verified CM05 result; no process attach or code edits. */
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
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"overlap_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-original+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
typedef struct {unsigned char *p;unsigned length,token;} Region;
static int canonical(void *p,unsigned o,const Region *regions,unsigned n){
 unsigned value=get(p,o);if(!value)return 1;
 for(unsigned i=0;i<n;i++)if(value>=address(regions[i].p)&&value-address(regions[i].p)<regions[i].length){put(p,o,regions[i].token+value-address(regions[i].p));return 1;}
 return 0;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;original=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot)return 5;memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) unsigned char seed[22000],first_owned[32+0x250+32],second_owned[32+0x250+32],root_owned[32+0xa80+32],pair_owned[32+0x50+32],cells_owned[32+16+32],body[0x430],manager[0x20],materials[0x40],mesh[16000],mesh_before[16000];
 unsigned char *first=first_owned+32,*second=second_owned+32,*root=root_owned+32,*pair=pair_owned+32,*cells=cells_owned+32;
 unsigned error=0,protected=0,mesh_bytes=0,tag=0,mask=0,capacity=0,seed_size=0;float shift[4];FILE *input=NULL;
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4];unsigned char saved[4][4096];
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(&tag,4,1,input)!=1||tag!=0x3130504d||fread(&mesh_bytes,4,1,input)!=1||mesh_bytes<0x60||mesh_bytes>16000||fread(shift,4,4,input)!=4||fread(&mask,4,1,input)!=1||fread(&capacity,4,1,input)!=1||capacity>1){error=7;goto done;}
 seed_size=0x500+mesh_bytes+0x1120;
 if(seed_size>sizeof(seed)||fread(seed,1,seed_size,input)!=seed_size||fgetc(input)!=EOF){error=7;goto done;}
 for(unsigned j=0;j<4;j++)if(!isfinite(shift[j])||fabsf(shift[j])>10000.f||(j==3&&shift[j]!=0.f)){error=8;goto done;}
 unsigned points=get(seed+0x500,0),faces=get(seed+0x500,4),face_start=0x60+points*48;
 if(points<4||points>80||faces<4||faces>110||face_start+faces*32>mesh_bytes||get(seed+0x500,0x54)!=0x60||get(seed+0x500,0x58)!=face_start||get(seed,0x10)!=7||get(seed,0x240)!=0x1500||seed[0x19]>31||get(seed,0x198)!=1){error=8;goto done;}
 for(unsigned i=0;i<points;i++)for(unsigned j=0;j<4;j++)if(!isfinite(num(seed+0x500,0x60+i*32+j*4))||fabsf(num(seed+0x500,0x60+i*32+j*4))>100000.f||(j==3&&num(seed+0x500,0x60+i*32+j*4)!=1.f)){error=8;goto done;}
 for(unsigned i=0;i<faces;i++)for(unsigned j=0;j<3;j++)if(get(seed+0x500,face_start+i*32+16+j*4)>=points){error=8;goto done;}
 for(unsigned j=0;j<16;j++)if(!isfinite(num(seed,0x130+j*4))||fabsf(num(seed,0x130+j*4))>10000.f){error=8;goto done;}
 if(!isfinite(num(seed,0x220))||num(seed,0x220)<0.f||num(seed,0x220)>1e6f||get(seed,0x194)>=1000000){error=8;goto done;}
 memset(first_owned,0xcd,sizeof(first_owned));memset(second_owned,0xcd,sizeof(second_owned));memset(root_owned,0xcd,sizeof(root_owned));memset(pair_owned,0xcd,sizeof(pair_owned));memset(cells_owned,0xcd,sizeof(cells_owned));
 memcpy(first,seed,0x250);memcpy(mesh,seed+0x500,mesh_bytes);memcpy(mesh_before,mesh,mesh_bytes);memset(second,0xcd,0x250);memset(root,0,0xa80);memset(pair,0,0x50);memset(cells,0,16);memset(body,0,sizeof(body));memset(manager,0,sizeof(manager));memset(materials,0,sizeof(materials));
 put(mesh,0x54,address(mesh+0x60));put(mesh,0x58,address(mesh+face_start));
 put(first,0,address(image+0xd3e7e0-0x400000));put(first,0xcc,address(root));put(first,0xc8,0);put(first,0xd8,address(body));put(first,0x240,address(mesh));put(first,0x21c,0);
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
 for(unsigned i=0;i<4;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,protections+i)){error=9;goto done;}++protected;}
 __declspec(align(16)) unsigned descriptor[33];memcpy(descriptor,seed+0x250,sizeof(descriptor));descriptor[0]=address(mesh);descriptor[0x5c/4]=address(root);((unsigned char *)descriptor)[0x50]=0;
 __try{
  ((void (__fastcall *)(void *,void *,void *))(image+0xb460f0-0x400000))(second,NULL,descriptor);
  memcpy(second+0x130,first+0x130,64);for(unsigned j=0;j<3;j++){float v=num(second,0x160+j*4)+shift[j];memcpy(second+0x160+j*4,&v,4);}
  ((void (__fastcall *)(void *,void *))(image+0xabc7b0-0x400000))(second,NULL);
  ((void (__fastcall *)(void *,void *,void *))(image+0xb16d70-0x400000))(root,NULL,second);
 }__except(fault(GetExceptionInformation())){error=10;}
 put(mesh,0x54,0x60);put(mesh,0x58,face_start);if(memcmp(mesh,mesh_before,mesh_bytes))error=11;
 if(!error){
  Region regions[]={{first,0x250,0x1000},{second,0x250,0xc000},{root,0xa80,0x12e0},{pair,0x50,0xd000},{cells,16,0xb000},{body,0x430,0xa000},{mesh,mesh_bytes,0x1500},{manager,0x20,0x1400},{materials,0x40,0x1420}};
  const unsigned pointers[]={0xb0,0xb4,0xb8,0xbc,0xc8,0xcc,0xd0,0xd4,0xd8,0xe0,0xe4,0xe8,0xec,0x170,0x174,0x178,0x17c,0x180,0x19c,0x1a0,0x1a4,0x1b0,0x1b4,0x1b8,0x1c4,0x1c8,0x1cc,0x1d8,0x1dc,0x1e0,0x1ec,0x1f0,0x1f4,0x200,0x204,0x208,0x21c,0x240};
  for(unsigned s=0;s<2;s++){unsigned char *shape=s?second:first;put(shape,0,0x7000);for(unsigned j=0;j<sizeof(pointers)/sizeof(pointers[0]);j++)if(!canonical(shape,pointers[j],regions,9))error=12;}
  const unsigned world_pointers[]={0x14,0x900,0x904,0x908,0x910,0x914,0x950,0x954};
  for(unsigned j=0;j<sizeof(world_pointers)/sizeof(world_pointers[0]);j++)if(!canonical(root,world_pointers[j],regions,9))error=12;
  for(unsigned j=0;j<16;j+=4)if(!canonical(cells,j,regions,9))error=12;
  const unsigned pair_pointers[]={4,8,0x28,0x2c,0x30,0x34,0x38,0x3c};
  for(unsigned j=0;j<sizeof(pair_pointers)/sizeof(pair_pointers[0]);j++)if(!canonical(pair,pair_pointers[j],regions,9))error=12;
  for(unsigned i=0;i<sizeof(body);i++)if(body[i])error=13;
  unsigned char *owners[]={first_owned,second_owned,root_owned,pair_owned,cells_owned};const unsigned sizes[]={0x250,0x250,0xa80,0x50,16};
  for(unsigned j=0;j<5;j++)for(unsigned i=0;i<32;i++)if(owners[j][i]!=0xcd||owners[j][32+sizes[j]+i]!=0xcd)error=13;
 }
done:
 if(input)fclose(input);
 for(unsigned i=0;i<protected;i++){DWORD ignored;unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);if(!VirtualProtect(page,4096,protections[i],&ignored))error=14;}
 unsigned unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,(sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData)))unchanged=0;
 if(!unchanged)error=14;
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=15;else{const void *parts[]={first,second,root,cells,pair};const DWORD lengths[]={0x250,0x250,0xa80,16,0x50};for(unsigned i=0;i<5;i++){DWORD written;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=16;}CloseHandle(output);}}
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);printf("mesh_overlap code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",unchanged,unchanged,error);return (int)error;
}
