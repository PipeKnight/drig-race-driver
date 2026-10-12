/* Whole original packed constructor with separately owned descriptor and sheets.
   No game process, allocator adapter, original entry point or code replacement. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <malloc.h>
#include <wchar.h>
#include "isolated_reference.h"
#include "owned_packed_shape_probe.h"
static unsigned word(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static unsigned addr(const void *p){return (unsigned)(uintptr_t)p;}
static int guarded(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+n+i]!=0xcd)return 0;return 1;}
static uintptr_t reference;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"packed_constructor_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[4];if(fread(h,4,4,file)!=4||h[0]!=0x32305350||h[1]<56||h[1]>2*1024*1024||(h[2]!=0&&h[2]!=0xcd)||h[3]>255){fclose(file);return 5;}
 unsigned size=h[1],fill=h[2],sheet=h[3];
 unsigned char *owned=_aligned_malloc(size+64,16),*before=malloc(size);
 if(!owned||!before){fclose(file);_aligned_free(owned);free(before);return 6;}
 memset(owned,0xcd,size+64);unsigned char *chunk=owned+32;
 if(fread(chunk,1,size,file)!=size||fgetc(file)!=EOF||ferror(file)){fclose(file);_aligned_free(owned);free(before);return 7;}fclose(file);
 Object admission;if(!prepare(chunk,size,&admission)){_aligned_free(owned);free(before);return 8;}
 unsigned nm=(unsigned)-(int)word(chunk,0x20);if(sheet>>nm){_aligned_free(owned);free(before);return 9;}
 for(unsigned i=0;i<nm;i++)for(unsigned j=0;j<i;j++)if(word(chunk,52+i*4)==word(chunk,52+j*4)&&((sheet>>i)&1)!=((sheet>>j)&1)){_aligned_free(owned);free(before);return 9;}
 memcpy(before,chunk,size);
 unsigned char *image=map_reference(argv[1]);if(!image){_aligned_free(owned);free(before);return 10;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned image_size=pe->OptionalHeader.SizeOfImage;unsigned char *snapshot=malloc(image_size);if(!snapshot)return 11;memcpy(snapshot,image,image_size);
 DWORD prior,ignored;if(!VirtualProtect(image+0xe58000-0x400000,4096,PAGE_READWRITE,&prior))return 12;
 __declspec(align(16)) unsigned char so[32+0x2c0+32],de[32+0x94+32],wo[32+0x100+32],mo[32+0x20+32],ma[32+0x200+32];
 unsigned char *shape=so+32,*descriptor=de+32,*world=wo+32,*manager=mo+32,*materials=ma+32;
 memset(so,0xcd,sizeof(so));memset(de,0xcd,sizeof(de));memset(wo,0xcd,sizeof(wo));memset(mo,0xcd,sizeof(mo));memset(ma,0xcd,sizeof(ma));
 memset(shape,fill,0x2c0);memset(descriptor,0,0x94);memset(world,0,0x100);memset(manager,0,0x20);memset(materials,0,0x200);
 put(world,0x14,addr(manager));put(world,0x80,27);put(manager,4,addr(materials));put(manager,8,nm);put(manager,12,0x3c);manager[0x10]=1;
 for(unsigned i=0;i<nm;i++){put(materials,i*0x3c,word(chunk,52+i*4));materials[i*0x3c+0x38]=(unsigned char)((sheet>>i)&1);}
 put(descriptor,0,addr(chunk));put(descriptor,4,0xdead2008);put(descriptor,0x5c,addr(world));put(descriptor,0x7c,word(chunk,52));
 unsigned error=0;
 __try{void *result=((void *(__fastcall *)(void *,void *,void *))(image+0xb1f900-0x400000))(shape,NULL,descriptor);if(result!=shape)error=13;}
 __except(fault(GetExceptionInformation())){error=14;}
 if(!error){
  if(word(shape,0)!=addr(image+0xd3d468-0x400000)||word(shape,0x10)!=11||word(shape,0x244)!=addr(chunk)||word(shape,0x240)!=0xdead2008||word(shape,0x290)!=sheet||word(world,0x80)!=28||word(shape,0xcc)!=addr(world)||word(shape,0xc8)!=addr(materials))error=15;
  for(unsigned o=0x294;o<=0x2a0;o+=4)if(word(shape,o)!=0x7f7fffff)error=16;
  if(memcmp(chunk,before,24)||memcmp(chunk+0x34+nm*4,before+0x34+nm*4,size-0x34-nm*4))error=17;
  if(!guarded(so,0x2c0)||!guarded(de,0x94)||!guarded(wo,0x100)||!guarded(mo,0x20)||!guarded(ma,0x200)||!guarded(owned,size))error=18;
 }
 memcpy(image+0xe58000-0x400000,snapshot+0xe58000-0x400000,4096);
 if(!VirtualProtect(image+0xe58000-0x400000,4096,prior,&ignored))error=19;
 if(memcmp(image,snapshot,image_size))error=20;
 if(!error){
  const unsigned fields[]={0xb0,0xb4,0xb8,0xbc,0x170,0x174,0x178,0x17c,0x180,0x1a4,0x1b8,0x1cc,0x1e0,0x1f4,0x208};
  for(unsigned i=0;i<sizeof(fields)/sizeof(fields[0]);i++){unsigned p=word(shape,fields[i]);if(p<addr(shape)||p-addr(shape)>=0x2c0){error=21;break;}put(shape,fields[i],0x1000+p-addr(shape));}
  for(unsigned i=0;i<4;i++){unsigned p=word(chunk,0x24+4*i),offset=word(before,0x24+4*i);if(p!=addr(chunk)+offset)error=22;put(chunk,0x24+4*i,0x100000+offset);}
  for(unsigned i=0;i<nm;i++){unsigned j=0;while(j<nm&&word(materials,j*0x3c)!=word(before,0x34+i*4))j++;if(j==nm)j=0;if(word(chunk,0x34+i*4)!=addr(materials)+j*0x3c)error=23;put(chunk,0x34+i*4,0x4000+j*0x3c);}
  put(shape,0,0x700b);put(shape,0xcc,0x2000);put(shape,0xc8,0x4000);put(shape,0x244,0x100000);put(descriptor,0,0x100000);put(descriptor,0x5c,0x2000);put(world,0x14,0x3000);put(manager,4,0x4000);
 }
 if(!error){
  HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
  if(output==INVALID_HANDLE_VALUE)error=24;
  else {const unsigned char *parts[]={shape,descriptor,world,manager,materials,chunk};const unsigned lengths[]={0x2c0,0x94,0x100,0x20,0x200,size};for(unsigned i=0;i<6&&!error;i++){DWORD written;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=25;}if(!CloseHandle(output))error=26;}
 }
 printf("packed_constructor code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",error!=20,error!=19&&error!=20,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);_aligned_free(owned);free(before);return error;
}
