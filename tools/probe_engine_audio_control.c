/* Coupled original RPM mapping, twelve pitches and twelve seek/rev lookups.
   Private isolated reference only. No backend calls, injection or helper stubs.
   Admitted domain excludes gear wobble and the diagnostic logger branch. */
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
#include "isolated_reference.h"
typedef void (__fastcall *Rpm)(void *,void *,const double *);
typedef void (__fastcall *Pitch)(void *,void *,unsigned);
typedef float (__fastcall *Revs)(void *,void *,const float *,int,unsigned,unsigned);
typedef unsigned (__fastcall *Seek)(void *,void *,const float *,float,unsigned,unsigned);
#pragma pack(push,1)
typedef struct {unsigned index; unsigned char controller[0x340];float speed,maximum,idle;double delta;} Input;
typedef struct {unsigned index;unsigned char controller[0x340];unsigned queries[24];} Output;
#pragma pack(pop)
static unsigned get(const void *p,unsigned at){unsigned v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static float scalar(const void *p,unsigned at){float v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static void put(void *p,unsigned at,unsigned v){memcpy((unsigned char *)p+at,&v,4);}
static int private_file(const wchar_t *path){
 wchar_t root[32768],full[32768];DWORD a=GetFullPathNameW(L"local\\",32768,root,NULL),b=GetFullPathNameW(path,32768,full,NULL);
 return a&&a<32768&&b&&b<32768&&!_wcsnicmp(root,full,wcslen(root));
}
static int guard(const Input *r,const float curves[4096]){
 if(!isfinite(r->speed)||!isfinite(r->maximum)||!isfinite(r->idle)||!isfinite(r->delta)||r->maximum<=0||r->speed<0||r->speed>r->maximum*2||r->idle<=0||r->idle>=r->maximum||r->delta<0||r->delta>1)return 30;
 const unsigned fields[]={0,0x38,0xb0,0xb4,0xc8,0x30c};
 for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++)if(!isfinite(scalar(r->controller,fields[i])))return 31;
 if(get(r->controller,0x28)!=0||scalar(r->controller,0x38)<0||scalar(r->controller,0x38)>2||scalar(r->controller,0)<0||scalar(r->controller,0)>2||fabsf(scalar(r->controller,0xb0))>2||fabsf(scalar(r->controller,0xb4))>1||fabsf(scalar(r->controller,0x30c))>1)return 31;
 /* Piecewise idle remapping differs from identity by at most endpoint gap. */
 if(fabsf(curves[1023]-r->idle/r->maximum)>.39f)return 32;
 for(unsigned i=0;i<12;i++){
  float position=scalar(r->controller,0x230+i*4);unsigned length=get(r->controller,0x2c0+i*4),rate=get(r->controller,0x290+i*4);
  float pitch=scalar(r->controller,0x260+i*4);
  if(!isfinite(position)||fabsf(position)>1000000000.f||!length||length>10000000||rate<8000||rate>192000||!isfinite(pitch)||pitch<.5f||pitch>2.f)return 33;
 }
 return 0;
}
static unsigned fault(EXCEPTION_POINTERS *p){fprintf(stderr,"exception=%08lx instruction=%p\n",p->ExceptionRecord->ExceptionCode,p->ExceptionRecord->ExceptionAddress);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
 if(argc!=4||!private_file(argv[1])||!private_file(argv[2])||!private_file(argv[3]))return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;
 memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned error=0,count=0,completed=0;
 float curves[4096];unsigned header[2];
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
 if(fread(header,8,1,input)!=1||header[0]!=0x31304345||!header[1]||header[1]>4096||fread(curves,sizeof(curves),1,input)!=1){error=6;goto done;}
 count=header[1];
 for(unsigned i=0;i<4096;i++)if(!isfinite(curves[i])||curves[i]<0||curves[i]>1||(i%2048<1024&&curves[i]<=0)){error=7;goto done;}
 HANDLE file=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
 if(file==INVALID_HANDLE_VALUE){error=8;goto done;}
 int descriptor=_open_osfhandle((intptr_t)file,_O_BINARY);
 if(descriptor<0){CloseHandle(file);error=8;goto done;}
 output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);error=8;goto done;}
 if(fwrite(header,8,1,output)!=1){error=9;goto done;}
 for(unsigned n=0;n<count;n++){
  Input r;Output result;
  if(fread(&r,sizeof(r),1,input)!=1){error=10;break;}
  error=(unsigned)guard(&r,curves);if(error)break;
  unsigned char context[0x380],profile[0x5a5c],component[0x80],vehicle[0x1900];
  memset(context,0xa5,sizeof(context));memset(profile,0x69,sizeof(profile));memset(component,0x6b,sizeof(component));memset(vehicle,0x71,sizeof(vehicle));
  unsigned char *controller=context+32;
  memcpy(controller,r.controller,0x340);memcpy(profile,curves,8192);memcpy(profile+0x3a5c,curves+2048,8192);
  put(controller,0x110,(unsigned)(uintptr_t)component);put(controller,0x118,(unsigned)(uintptr_t)profile);
  put(component,0x24,(unsigned)(uintptr_t)vehicle);
  memcpy(vehicle+0x1894,&r.speed,4);memcpy(vehicle+0x17a4,&r.maximum,4);memcpy(vehicle+0x17a8,&r.idle,4);
  unsigned char old_profile[sizeof(profile)],old_component[sizeof(component)],old_vehicle[sizeof(vehicle)];
  memcpy(old_profile,profile,sizeof(profile));memcpy(old_component,component,sizeof(component));memcpy(old_vehicle,vehicle,sizeof(vehicle));
  __try {
   ((Rpm)(image+0x422a40-0x400000))(controller,NULL,&r.delta);
   put(controller,0xc8,get(controller,0x48));
   ((Rpm)(image+0x4152a0-0x400000))(controller,NULL,&r.delta);
   for(unsigned category=0;category<3;category++)((Pitch)(image+0x415500-0x400000))(controller,NULL,category);
   for(unsigned bank=0;bank<4;bank++)for(unsigned category=0;category<3;category++){
    unsigned slot=bank*3+category;float *table=(float *)(profile+(category?0x3a5c:0));
    float rev=((Revs)(image+0x4153e0-0x400000))(controller,NULL,table,(int)scalar(controller,0x230+slot*4),bank,category);
    result.queries[slot*2]=bits(rev);
    result.queries[slot*2+1]=((Seek)(image+0x415460-0x400000))(controller,NULL,table+1024,scalar(controller,0xc8),bank,category);
   }
  }__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  for(unsigned i=0;i<32;i++)if(context[i]!=0xa5||context[0x360+i]!=0xa5)error=21;
  if(memcmp(profile,old_profile,sizeof(profile))||memcmp(component,old_component,sizeof(component))||memcmp(vehicle,old_vehicle,sizeof(vehicle)))error=22;
  if(get(controller,0x110)!=(unsigned)(uintptr_t)component||get(controller,0x118)!=(unsigned)(uintptr_t)profile)error=23;
  if(error)break;
  put(controller,0x110,get(r.controller,0x110));put(controller,0x118,get(r.controller,0x118));
  result.index=r.index;memcpy(result.controller,controller,0x340);
  if(fwrite(&result,sizeof(result),1,output)!=1){error=24;break;}completed++;
 }
 if(!error&&fgetc(input)!=EOF)error=25;
done:
 if(input)fclose(input);if(output&&fclose(output))error=26;
 unsigned unchanged=memcmp(image,saved,pe->OptionalHeader.SizeOfImage)==0;
 if(!unchanged)error=27;
 printf("engine_audio_control_cases=%u completed=%u entire_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(saved);VirtualFree(image,0,MEM_RELEASE);return (int)error;
}
