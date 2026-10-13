/* Whole original9A2A70/989490/98A7D0 on owned bounded source records.
   No entry point/import/TLS execution or replacement of original helpers. */
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
#include <xmmintrin.h>
#include "isolated_reference.h"
#define DESCRIPTOR 0xa500
static unsigned get(const void *p,unsigned o){unsigned w;memcpy(&w,(const char *)p+o,4);return w;}
static void put(void *p,unsigned o,unsigned w){memcpy((char *)p+o,&w,4);}
static float number(const void *p,unsigned o){float x;memcpy(&x,(const char *)p+o,4);return x;}
static int range(float x,float lo,float hi){return isfinite(x)&&x>=lo&&x<=hi;}
static int admitted(const unsigned char *r){
 unsigned n=get(r,0);if(n>32||get(r,8)>255||!range(number(r,12),0.f,10.f)||!range(number(r,16),.005f,1.f)||!range(number(r,20),1.001f,2.f)||!range(number(r,24),-100.f,-.001f)||!range(number(r,28),-100.f,-.001f))return 0;
 float lo[4]={FLT_MAX,FLT_MAX,FLT_MAX,1.f},hi[4]={-FLT_MAX,-FLT_MAX,-FLT_MAX,1.f};
 for(unsigned i=0;i<n;i++)for(unsigned a=0;a<3;a++){
  float x=number(r+32+DESCRIPTOR+i*32,a*4),y=number(r+32+DESCRIPTOR+i*32,16+a*4);
  if(!range(x,-1000.f,1000.f)||!range(y,-1000.f,1000.f))return 0;
  if(x<lo[a])lo[a]=x;if(hi[a]<y)hi[a]=y;
 }
 for(unsigned a=0;a<3;a++){lo[a]-=number(r,12);hi[a]+=number(r,12);}
 if(hi[0]<lo[0]){lo[0]=lo[1]=number(r,24);lo[2]=number(r,28);lo[3]=0;for(unsigned a=0;a<4;a++)hi[a]=number(r,24)*lo[a];}
 float d[4];for(unsigned a=0;a<4;a++){d[a]=hi[a]-lo[a];if(!isfinite(d[a])||d[a]<0)return 0;}
 float square=(d[1]*d[1]+d[0]*d[0])+(d[3]*d[3]+d[2]*d[2]);if(!isnormal(square)||square<=0)return 0;
 float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(square))),product=square*seed;
 float spacing=((1.f-product*seed)*(product*.5f)+product)*number(r,16);
 for(unsigned iteration=0;iteration<4096;iteration++){
  spacing*=number(r,20);if(!isnormal(spacing)||spacing<=0)return 0;
  unsigned dims[3];float inverse=1.f/spacing;
  for(unsigned a=0;a<3;a++){float ratio=inverse*d[a];if(!range(ratio,0.f,1000.f))return 0;dims[a]=(unsigned)ratio+2;}
  if(dims[0]*dims[1]*dims[2]<=400)return 1;
 }
 return 0;
}
static int sentinels(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[n-32+i]!=0xcd)return 0;return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"layout_fault=%08lx eip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 if(fseek(file,0,SEEK_END)){fclose(file);return 5;}long size=ftell(file);
 if(size<8||size>8*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 5;}
 unsigned char *input=malloc((size_t)size);if(!input){fclose(file);return 6;}
 int valid=fread(input,1,(size_t)size,file)==(size_t)size;fclose(file);
 unsigned count=valid?get(input,4):0;if(!valid||get(input,0)!=0x31304c47||!count||count>128){free(input);return 7;}
 size_t cursor=8;unsigned error=0,completed=0;
 for(unsigned i=0;i<count;i++){
  if(!inside(cursor,32+DESCRIPTOR,(size_t)size)){error=8;break;}
  unsigned n=get(input+cursor,0);if(n>32||!inside(cursor,32+DESCRIPTOR+n*32,(size_t)size)){error=8;break;}
  if(!admitted(input+cursor)){error=9;break;}cursor+=32+DESCRIPTOR+n*32;
 }
 if(!error&&cursor!=(size_t)size)error=8;
 if(error){free(input);return error;}
 unsigned char *results=malloc(8+count*DESCRIPTOR);if(!results){free(input);return 6;}memcpy(results,input,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(input);free(results);return 10;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *saved=malloc(image_size);if(!saved){free(input);free(results);VirtualFree(image,0,MEM_RELEASE);return 6;}memcpy(saved,image,image_size);
 const unsigned pages[]={0xcaf000,0xcb0000,0xce2000,0xcf7000,0xe4f000,0xe58000,0xe8f000};DWORD previous[7],ignored;unsigned protected=0;
 for(unsigned i=0;i<7;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,previous+i)){error=11;break;}protected++;}
 cursor=8;
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=input+cursor;unsigned n=get(r,0);
  for(unsigned j=0;j<7;j++)memcpy(image+pages[j]-0x400000,saved+pages[j]-0x400000,4096);
  const unsigned addresses[]={0xcb0de4,0xcafe5c,0xce2bb4,0xcb0948,0xcf7718};
  for(unsigned j=0;j<5;j++)put(image,addresses[j]-0x400000,get(r,12+j*4));image[0xe4fc34-0x400000]=(unsigned char)get(r,8);
  __declspec(align(16)) unsigned char descriptor[32+DESCRIPTOR+32],owner[32+0x450+32],shapes[32][32+0x130+32];
  unsigned char owner_before[sizeof(owner)],shapes_before[sizeof(shapes)];
  memset(descriptor,0xcd,sizeof(descriptor));memcpy(descriptor+32,r+32,DESCRIPTOR);
  memset(owner,0xcd,sizeof(owner));memset(owner+32,0,0x450);memset(shapes,0xcd,sizeof(shapes));
  put(owner+32,0x44,n);put(owner+32,0x44c,get(r,4));
  for(unsigned j=0;j<n;j++){put(owner+32,0x48+j*4,(unsigned)(uintptr_t)(shapes[j]+32));memcpy(shapes[j]+32+0x110,r+32+DESCRIPTOR+j*32,32);}
  memcpy(owner_before,owner,sizeof(owner));memcpy(shapes_before,shapes,sizeof(shapes));
  __try{((void (__fastcall *)(void *,void *,void *))(image+0x9a2a70-0x400000))(descriptor+32,NULL,owner+32);}__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  if(!sentinels(descriptor,sizeof(descriptor))||memcmp(owner,owner_before,sizeof(owner))||memcmp(shapes,shapes_before,sizeof(shapes))){error=21;break;}
  memcpy(results+8+i*DESCRIPTOR,descriptor+32,DESCRIPTOR);completed++;
  cursor+=32+DESCRIPTOR+n*32;
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,previous[i],&ignored))error=22;}
 unsigned unchanged=memcmp(image,saved,image_size)==0;if(!unchanged)error=23;
 if(!error){
  HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
  if(h==INVALID_HANDLE_VALUE)error=24;else{
   DWORD written=0;unsigned length=8+count*DESCRIPTOR;if(!WriteFile(h,results,length,&written,NULL)||written!=length)error=25;
   if(!CloseHandle(h))error=25;if(error)_wremove(argv[3]);
  }
 }
 printf("layout_cases=%u completed=%u owned_canaries=1 full_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(input);free(results);free(saved);VirtualFree(image,0,MEM_RELEASE);return error;
}
