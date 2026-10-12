/* Whole original CQTC constructor and vertex/triangle decoders on owned storage.
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

static unsigned word(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static unsigned addr(const void *p){return (unsigned)(uintptr_t)p;}
static int guarded(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+n+i]!=0xcd)return 0;return 1;}
static uintptr_t reference;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"cqtc_decode_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[4];if(fread(h,4,4,file)!=4||h[0]!=0x31305143||h[1]<56||h[1]>2*1024*1024||(h[2]!=0&&h[2]!=0xcd)||h[3]>255){fclose(file);return 5;}
 unsigned size=h[1],fill=h[2],sheet=h[3];
 unsigned char *owned=_aligned_malloc(size+64,16),*before=malloc(size);
 if(!owned||!before){fclose(file);_aligned_free(owned);free(before);return 6;}
 memset(owned,0xcd,size+64);unsigned char *chunk=owned+32;
 if(fread(chunk,1,size,file)!=size||fgetc(file)!=EOF||ferror(file)){fclose(file);_aligned_free(owned);free(before);return 7;}fclose(file);
 unsigned nt=word(chunk,24),nv=word(chunk,28),nm=word(chunk,32),vo=word(chunk,36),no=word(chunk,40),to=word(chunk,44),ro=word(chunk,48);
 if(!nt||nt>32768||!nv||nv>65535||!nm||nm>8||vo!=52+nm*4||no!=vo+nv*8||to<=no||to>size||(to-no)%3||ro!=to+nt*7||ro>size){_aligned_free(owned);free(before);return 8;}
 for(unsigned j=0;j<3;j++){float low,high;memcpy(&low,chunk+j*4,4);memcpy(&high,chunk+12+j*4,4);if(!isfinite(low)||!isfinite(high)||low>=high){_aligned_free(owned);free(before);return 8;}}
 for(unsigned i=0;i<nt;i++){const unsigned char *t=chunk+to+i*7;unsigned a=((unsigned)t[0]<<16)|((unsigned)t[1]<<8)|t[2],b=a+t[4]+((unsigned)(t[3]&240)<<4),c=a+t[5]+((unsigned)(t[3]&15)<<8);if(a>=nv||b>=nv||c>=nv||t[6]>=nm){_aligned_free(owned);free(before);return 8;}}
if(sheet>>nm){_aligned_free(owned);free(before);return 9;}
 for(unsigned i=0;i<nm;i++)for(unsigned j=0;j<i;j++)if(word(chunk,52+i*4)==word(chunk,52+j*4)&&((sheet>>i)&1)!=((sheet>>j)&1)){_aligned_free(owned);free(before);return 9;}
 memcpy(before,chunk,size);
 unsigned char *image=map_reference(argv[1]);if(!image){_aligned_free(owned);free(before);return 10;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned image_size=pe->OptionalHeader.SizeOfImage;unsigned char *snapshot=malloc(image_size);if(!snapshot)return 11;memcpy(snapshot,image,image_size);
 DWORD prior,ignored;if(!VirtualProtect(image+0xe58000-0x400000,4096,PAGE_READWRITE,&prior))return 12;
 __declspec(align(16)) unsigned char so[32+0x270+32],de[32+0x94+32],wo[32+0x100+32],mo[32+0x20+32],ma[32+0x200+32];
 unsigned char *shape=so+32,*descriptor=de+32,*world=wo+32,*manager=mo+32,*materials=ma+32;
 memset(so,0xcd,sizeof(so));memset(de,0xcd,sizeof(de));memset(wo,0xcd,sizeof(wo));memset(mo,0xcd,sizeof(mo));memset(ma,0xcd,sizeof(ma));
 memset(shape,fill,0x270);memset(descriptor,0,0x94);memset(world,0,0x100);memset(manager,0,0x20);memset(materials,0,0x200);
 put(world,0x14,addr(manager));put(world,0x80,27);put(manager,4,addr(materials));put(manager,8,nm);put(manager,12,0x3c);manager[0x10]=1;
 for(unsigned i=0;i<nm;i++){put(materials,i*0x3c,word(chunk,52+i*4));materials[i*0x3c+0x38]=(unsigned char)((sheet>>i)&1);}
 put(descriptor,0,addr(chunk));put(descriptor,4,0xdead2008);put(descriptor,0x5c,addr(world));put(descriptor,0x7c,word(chunk,52));
 unsigned error=0;
 __try{void *result=((void *(__fastcall *)(void *,void *,void *))(image+0xb1b0c0-0x400000))(shape,NULL,descriptor);if(result!=shape)error=13;}
 __except(fault(GetExceptionInformation())){error=14;}
 if(!error){
  if(word(shape,0)!=addr(image+0xd3d494-0x400000)||word(shape,0x10)!=10||word(shape,0x244)!=addr(chunk)||word(shape,0x240)!=0xdead2008||word(world,0x80)!=28||word(shape,0xcc)!=addr(world)||word(shape,0xc8)!=addr(materials))error=15;
  if(memcmp(chunk+24,before+24,12))error=16;
  if(memcmp(chunk,before,24)||memcmp(chunk+0x34+nm*4,before+0x34+nm*4,size-0x34-nm*4))error=17;
  if(!guarded(so,0x270)||!guarded(de,0x94)||!guarded(wo,0x100)||!guarded(mo,0x20)||!guarded(ma,0x200)||!guarded(owned,size))error=18;
 }
 unsigned decoded_size=(nv+nt)*16;
 unsigned char *decoded_owned=malloc((size_t)decoded_size+64),*decoded=decoded_owned?decoded_owned+32:NULL;
 if(!decoded)error=27;else memset(decoded_owned,0xcd,(size_t)decoded_size+64);
 if(!error){
  __try{
   for(unsigned i=0;i<nv;i++){
    __declspec(align(16)) unsigned char v_owned[80];memset(v_owned,0xcd,sizeof(v_owned));float *v=(float *)(v_owned+32);
    ((void *(__fastcall *)(void *,void *,void *,void *))(image+0xb1aa90-0x400000))(shape,NULL,v,chunk+vo+i*8);
    for(unsigned j=0;j<4;j++)if(!isfinite(v[j]))error=28;
    if(!guarded(v_owned,16))error=33;
    memcpy(decoded+i*16,v,16);
   }
   for(unsigned i=0;i<nt;i++){
    unsigned *row=(unsigned *)(decoded+nv*16+i*16);unsigned p_owned[19];memset(p_owned,0xcd,sizeof(p_owned));unsigned *pointers=p_owned+8;const unsigned char *t=chunk+to+i*7;
    ((void (__fastcall *)(void *,void *,void *,void *))(image+0xb1a4c0-0x400000))((void *)t,NULL,chunk,pointers);
    for(unsigned j=0;j<3;j++){if(pointers[j]<addr(chunk)+vo||(pointers[j]-addr(chunk)-vo)%8||(pointers[j]-addr(chunk)-vo)/8>=nv)error=29;row[j]=(pointers[j]-addr(chunk)-vo)/8;}
    if(!guarded((unsigned char *)p_owned,12))error=34;
    unsigned material=((unsigned (__fastcall *)(void *,void *,void *))(image+0xb1a4a0-0x400000))((void *)t,NULL,chunk);
    if(material<addr(materials)||(material-addr(materials))%0x3c||(material-addr(materials))/0x3c>=nm)error=30;
    else row[3]=word((void *)(uintptr_t)material,0);
   }
  }__except(fault(GetExceptionInformation())){error=31;}
  if(!guarded(so,0x270)||!guarded(de,0x94)||!guarded(wo,0x100)||!guarded(mo,0x20)||!guarded(ma,0x200)||!guarded(owned,size))error=32;
  if(!guarded(decoded_owned,decoded_size))error=35;
 }
 memcpy(image+0xe58000-0x400000,snapshot+0xe58000-0x400000,4096);
 if(!VirtualProtect(image+0xe58000-0x400000,4096,prior,&ignored))error=19;
 if(memcmp(image,snapshot,image_size))error=20;
 if(!error){
  const unsigned fields[]={0xb0,0xb4,0xb8,0xbc,0x170,0x174,0x178,0x17c,0x180,0x1a4,0x1b8,0x1cc,0x1e0,0x1f4,0x208};
  for(unsigned i=0;i<sizeof(fields)/sizeof(fields[0]);i++){unsigned p=word(shape,fields[i]);if(p<addr(shape)||p-addr(shape)>=0x270){error=21;break;}put(shape,fields[i],0x1000+p-addr(shape));}
  for(unsigned i=0;i<4;i++){unsigned p=word(chunk,0x24+4*i),offset=word(before,0x24+4*i);if(p!=addr(chunk)+offset)error=22;put(chunk,0x24+4*i,0x100000+offset);}
  for(unsigned i=0;i<nm;i++){unsigned j=0;while(j<nm&&word(materials,j*0x3c)!=word(before,0x34+i*4))j++;if(j==nm)j=0;if(word(chunk,0x34+i*4)!=addr(materials)+j*0x3c)error=23;put(chunk,0x34+i*4,0x4000+j*0x3c);}
  put(shape,0,0x700a);put(shape,0xcc,0x2000);put(shape,0xc8,0x4000);put(shape,0x244,0x100000);put(descriptor,0,0x100000);put(descriptor,0x5c,0x2000);put(world,0x14,0x3000);put(manager,4,0x4000);
 }
 if(!error){
  HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
  if(output==INVALID_HANDLE_VALUE)error=24;
  else {const unsigned char *parts[]={shape,descriptor,world,manager,materials,chunk,decoded};const unsigned lengths[]={0x270,0x94,0x100,0x20,0x200,size,(nv+nt)*16};for(unsigned i=0;i<7&&!error;i++){DWORD written;if(!WriteFile(output,parts[i],lengths[i],&written,NULL)||written!=lengths[i])error=25;}if(!CloseHandle(output))error=26;}
 }
 printf("cqtc_decode code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",error!=20,error!=19&&error!=20,error);
 free(decoded_owned);free(snapshot);VirtualFree(image,0,MEM_RELEASE);_aligned_free(owned);free(before);return error;
}
