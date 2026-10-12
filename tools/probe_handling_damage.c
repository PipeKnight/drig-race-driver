/* Whole unchanged 9F3E80 with reached 9E6660 calls, explicit global RNG,
   owned immutable query records, car and physical wrapper. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <wchar.h>
#include "isolated_reference.h"
#define CAR 0x21000
#define PHYSICAL 0x424
#define PREFIX (16+64*8)
#define SIZE (PREFIX+CAR+PHYSICAL)
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
#define GRID_HANDLING_QUERIES
#define GRID_HANDLING_DAMAGE
#include "vehicle_handling_probe.h"
#include "handling_query_probe.h"
static int range(const void *p,unsigned o,float lo,float hi){float v=value(p,o);return isfinite(v)&&v>=lo&&v<=hi;}
static int canary(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+n+i]!=0xcd)return 0;return 1;}
static int damage_owner(const unsigned char *v,const unsigned char *p){
 const unsigned char *b=v+MUTABLE;
 if(get(b,0x210)||v[0x104d]>1||!range(p,0x3f0,1.f,1.e6f)||value(p,0x20c)!=1.f)return 0;
 for(unsigned j=0;j<16;j++)if(!range(p,0x1d0+j*4,-1000.f,1000.f))return 0;
 for(unsigned j=0;j<3;j++)for(unsigned k=0;k<4;k++)if(k==3?get(p,0x1d0+j*16+k*4)!=0:!range(p,0x1d0+j*16+k*4,-1.1f,1.1f))return 0;
 for(unsigned j=0x430;j<0x43c;j+=4)if(!range(b,j,0.f,1000.f))return 0;
 if(!range(b,0x42c,0.f,1.5f))return 0;
 for(unsigned j=0;j<6;j++)if(!range(b,j*4,0.f,1.5f))return 0;
 for(unsigned j=0;j<4;j++)if(!range(b,0x28+j*4,0.f,1.5f))return 0;
 for(unsigned j=0x360;j<0x37c;j+=4)if(!range(b,j,0.f,1000.f))return 0;
 float gain=(value(b,0x438)*value(b,0x430))*value(b,0x434),secondary=(value(b,0x438)*value(b,0x434))*value(b,0x430);
 if(!isfinite(gain)||gain>1000.f||!isfinite(secondary)||secondary>1000.f||value(b,0x36c)<=0.f)return 0;
 const unsigned boxes[]={0x60,0x90,0xc0,0xf0,0x120,0x150,0x180,0x1b0,0x1e0};
 for(unsigned i=0;i<9;i++){
  unsigned at=boxes[i];if(b[at+32]>1||(value(b,at+12)!=0.f&&value(b,at+12)!=1.f)||(value(b,at+28)!=0.f&&value(b,at+28)!=1.f))return 0;
  for(unsigned j=0;j<3;j++)if(!range(b,at+j*4,-10000.f,10000.f)||!range(b,at+16+j*4,0.f,100.f))return 0;
 }
 const unsigned coeff[]={0x218,0x22c,0x240,0x304,0x32c,0x340};
 for(unsigned i=0;i<6;i++)if(!range(b,coeff[i],0.f,1000.f)||!range(b,coeff[i]+4,0.f,1.f)||!range(b,coeff[i]+8,0.f,1.f))return 0;
 return 1;
}
static int admitted(const unsigned char *r){
 const unsigned char *v=r+PREFIX,*p=v+CAR,*b=v+MUTABLE;HandlingQueries queries;unsigned char receiver[HANDLING_QUERY_BYTES];
 memcpy(&queries,r+8,sizeof(queries));if(query_receiver(&queries,receiver)||handling_guard(v,p,value(r,0))||get(b,0x210))return 0;
 float delta=value(r,0);if(delta>.02f)delta=.02f;
 float threshold=(float)((double)value(p,0x3f0)*(double)value(b,0x368));
 for(unsigned i=0;i<4;i++)if(value(v,0x1ac8+i*0x150)*delta>threshold){
  if(!damage_owner(v,p)||value(v,0x1ac8+i*0x150)*delta>1.e9f)return 0;
  unsigned at=0x60+i*48;float local[4],point[4];for(unsigned j=0;j<4;j++)local[j]=value(b,at+j*4);
  for(unsigned j=0;j<4;j++){point[j]=(local[3]*value(p,0x200+j*4)+local[1]*value(p,0x1e0+j*4))+(local[2]*value(p,0x1f0+j*4)+local[0]*value(p,0x1d0+j*4));if(!isfinite(point[j])||fabsf(point[j])>1000.f)return 0;}
  if(point[3]!=0.f&&point[3]!=1.f)return 0;
 }
 return 1;
}
static uintptr_t reference;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"handling_damage_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[2];if(fread(h,4,2,file)!=2||h[0]!=0x32304856||!h[1]||h[1]>64){fclose(file);return 5;}
 unsigned count=h[1];unsigned char *inputs=malloc(count*SIZE),*outputs=malloc(8+count*SIZE);
 if(!inputs||!outputs){fclose(file);free(inputs);free(outputs);return 6;}
 int valid=fread(inputs,1,count*SIZE,file)==count*SIZE&&fgetc(file)==EOF;fclose(file);
 if(!valid){free(inputs);free(outputs);return 7;}
 for(unsigned i=0;i<count;i++)if(!admitted(inputs+i*SIZE)){free(inputs);free(outputs);return 8;}
 memcpy(outputs,h,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(inputs);free(outputs);return 9;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *snapshot=malloc(image_size);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return 10;}memcpy(snapshot,image,image_size);
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xe85000,0xe4f000};DWORD prior[6],ignored;unsigned protected=0,error=0;
 for(unsigned i=0;i<6;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,prior+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=inputs+i*SIZE;
  __declspec(align(16)) unsigned char owner[32+CAR+32],physical[32+0x430+32],component[32+0x100+32],wrapper[32+0x10+32],query[32+HANDLING_QUERY_BYTES+32];
  unsigned char *v=owner+32,*p=physical+32,*c=component+32,*w=wrapper+32,*q=query+32;HandlingQueries queries;unsigned char query_before[HANDLING_QUERY_BYTES];
  memset(owner,0xcd,sizeof(owner));memset(physical,0xcd,sizeof(physical));memset(component,0xcd,sizeof(component));memset(wrapper,0xcd,sizeof(wrapper));memset(query,0xcd,sizeof(query));
  memcpy(v,r+PREFIX,CAR);memset(p,0,0x430);memcpy(p,r+PREFIX+CAR,PHYSICAL);memset(c,0,0x100);memset(w,0,0x10);memcpy(&queries,r+8,sizeof(queries));
  if(query_receiver(&queries,q)){error=8;break;}memcpy(query_before,q,sizeof(query_before));
  put(w,8,(unsigned)(uintptr_t)p);put(v,0xdb0,(unsigned)(uintptr_t)w);put(c,0x24,(unsigned)(uintptr_t)v);put(v,MUTABLE+0x214,(unsigned)(uintptr_t)c);if(queries.present)put(c,0x10,(unsigned)(uintptr_t)q);
  for(unsigned j=0;j<6;j++)memcpy(image+pages[j]-0x400000,snapshot+pages[j]-0x400000,4096);
  put(image,0xe4fcc0-0x400000,get(r,4));
  __try{((void (__fastcall *)(void *,void *,float))(image+0x9f3e80-0x400000))(v+MUTABLE,NULL,value(r,0));}__except(fault(GetExceptionInformation())){error=12;}
  if(get(w,8)!=(unsigned)(uintptr_t)p||get(v,0xdb0)!=(unsigned)(uintptr_t)w||get(c,0x24)!=(unsigned)(uintptr_t)v||get(v,MUTABLE+0x214)!=(unsigned)(uintptr_t)c||get(c,0x10)!=(queries.present?(unsigned)(uintptr_t)q:0)||get(v,MUTABLE+0x210))error=13;
  put(v,0xdb0,0);put(v,MUTABLE+0x214,0);put(c,0x10,0);put(w,8,0);put(c,0x24,0);
  if(!canary(owner,CAR)||!canary(physical,0x430)||!canary(component,0x100)||!canary(wrapper,0x10)||!canary(query,HANDLING_QUERY_BYTES)||memcmp(p,r+PREFIX+CAR,PHYSICAL)||memcmp(query_before,q,sizeof(query_before)))error=13;
  for(unsigned j=PHYSICAL;j<0x430;j++)if(p[j])error=13;
  for(unsigned j=0;j<CAR;j++)if((j<MUTABLE||j>=MUTABLE+0x460)&&(j<0x1644||j>=0x1648)&&v[j]!=r[PREFIX+j])error=13;
  for(unsigned j=0;j<0x100;j++)if(c[j])error=13;
  for(unsigned j=0;j<0x10;j++)if(w[j])error=13;
  if(!bounded(v+MUTABLE,0,0x60)||!bounded(v+MUTABLE,0x42c,0x430))error=14;
  for(unsigned j=0x3a0;j<0x3c4;j+=4)if(!isfinite(value(v+MUTABLE,j)))error=14;
  for(unsigned j=0;j<4;j++)for(unsigned k=0;k<5;k++)if(!isfinite(value(v+MUTABLE,0x3c8+j*24+k*4)))error=14;
  unsigned char *out=outputs+8+i*SIZE;memcpy(out,r,PREFIX);put(out,4,get(image,0xe4fcc0-0x400000));memcpy(out+PREFIX,v,CAR);memcpy(out+PREFIX+CAR,p,PHYSICAL);
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,prior[i],&ignored))error=15;}
 if(memcmp(image,snapshot,image_size))error=16;
 if(!error){HANDLE out=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(out==INVALID_HANDLE_VALUE)error=17;else{DWORD written;if(!WriteFile(out,outputs,8+count*SIZE,&written,NULL)||written!=8+count*SIZE)error=18;if(!CloseHandle(out))error=19;}}
 printf("handling_damage_cases=%u code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",count,error!=16,error!=15&&error!=16,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return error;
}
