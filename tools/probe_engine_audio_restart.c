/* Coupled original RPM mapping, twelve pitches and twelve seek/rev lookups.
   Private isolated reference only. No live injection; supplied virtual playback service only.
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

/* Unchanged 423210 plus real table helpers. Virtual service is supplied,
   records play requests and returns authored accepted-seek values. Not OpenAL. */
typedef void (__fastcall *Restart)(void *,void *,unsigned);
#pragma pack(push,1)
typedef struct {unsigned index;unsigned char controller[0x340];unsigned category,accepted[2];float cooldown;unsigned resource;} Input;
typedef struct {unsigned slot,resource,sample,flags,group,requested,accepted;} Command;
typedef struct {unsigned index;unsigned char controller[0x340];unsigned count;Command commands[2];} Output;
#pragma pack(pop)
static unsigned get(const void *p,unsigned at){unsigned v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static float scalar(const void *p,unsigned at){float v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static void put(void *p,unsigned at,unsigned v){memcpy((unsigned char *)p+at,&v,4);}
static int private_file(const wchar_t *path){
 wchar_t root[32768],full[32768];DWORD a=GetFullPathNameW(L"local\\",32768,root,NULL),b=GetFullPathNameW(path,32768,full,NULL);
 return a&&a<32768&&b&&b<32768&&!_wcsnicmp(root,full,wcslen(root));
}

static Output *active;
static const Input *request;
static unsigned char *active_controller;
static unsigned bad_service;
static unsigned __fastcall group_service(void *self,void *unused,unsigned group){
 (void)self;(void)unused;return group^0x100;
}
static void __fastcall play_service(void *self,void *unused,void **voice,unsigned resource,unsigned sample,unsigned flags,unsigned group,unsigned *seek){
 (void)self;(void)unused;
 if(active->count>=2){bad_service=1;return;}
 uintptr_t start=(uintptr_t)(active_controller+0x1ec),at=(uintptr_t)voice;
 if(at<start||at>=start+48||(at-start)%4){bad_service=1;return;}
 Command *c=active->commands+active->count;
 c->slot=(unsigned)((at-start)/4);c->resource=resource;c->sample=sample;c->flags=flags;c->group=group;c->requested=*seek;
 unsigned accepted=request->accepted[active->count];
 if(accepted!=0xffffffff)*seek=accepted;
 c->accepted=*seek;active->count++;
}
static unsigned guard(const Input *r,const float *curves){
 if(r->category>2||!isfinite(r->cooldown)||r->cooldown<0||r->cooldown>1)return 30;
 for(unsigned i=0;i<2;i++)if(r->accepted[i]!=0xffffffff&&r->accepted[i]>60000000)return 30;
 for(unsigned i=0;i<4096;i++)if(!isfinite(curves[i])||curves[i]<0||curves[i]>1||(i%2048<1024&&curves[i]<=0))return 31;
 unsigned c=r->category;
 if(!isfinite(scalar(r->controller,0xc8))||scalar(r->controller,0xc8)<0||scalar(r->controller,0xc8)>1||!isfinite(scalar(r->controller,0x134+c*4))||fabsf(scalar(r->controller,0x134+c*4))>60||r->controller[0x14c+c]>1||r->controller[0x14f+c]>1)return 32;
 unsigned outgoing=get(r->controller,0x1d4+c*4),incoming=get(r->controller,0x1e0+c*4);
 if((outgoing!=0&&outgoing!=2)||incoming!=2-outgoing)return 33;
 for(unsigned b=0;b<4;b++){unsigned slot=b*3+c,l=get(r->controller,0x2c0+slot*4),rate=get(r->controller,0x290+slot*4);if(!l||l>10000000||rate<8000||rate>192000)return 34;}
 if(r->controller[0x14c+c]&&scalar(r->controller,0x134+c*4)<0){
  unsigned base=c?2048:0,slot=outgoing*3+c,idx=(unsigned)(scalar(r->controller,0xc8)*1024.f);if(idx>1023)idx=1023;
  unsigned requested=(unsigned)((double)get(r->controller,0x2c0+slot*4)*(double)curves[base+1024+idx]/(double)get(r->controller,0x290+slot*4)*1000000.);
  unsigned accepted=r->accepted[0]==0xffffffff?requested:r->accepted[0];slot=incoming*3+c;
  float position=(float)(((double)accepted*(double)1e-6f)*(double)get(r->controller,0x290+slot*4));
  float time=(float)((double)(int)position/(double)get(r->controller,0x2c0+slot*4));if(time>1)time=1;
  idx=(unsigned)(time*1024.f);if(idx>1023)idx=1023;
  if((double)scalar(r->controller,0xc8)/(double)curves[base+idx]>1.5)return 35;
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
 unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
 const unsigned pages[]={0xe50000,0xe5a000};DWORD protections[2];unsigned protected=0,error=0,count=0,completed=0;
 for(unsigned i=0;i<2;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=5;break;}protected++;}
 FILE *input=NULL,*output=NULL;unsigned header[2];float curves[4096];
 if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(header,8,1,input)!=1||header[0]!=0x31305245||!header[1]||header[1]>4096||fread(curves,sizeof(curves),1,input)!=1){error=7;goto done;}count=header[1];
 HANDLE file=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
 if(file==INVALID_HANDLE_VALUE){error=8;goto done;}
 int descriptor=_open_osfhandle((intptr_t)file,_O_BINARY);if(descriptor<0){CloseHandle(file);error=8;goto done;}
 output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);error=8;goto done;}
 if(fwrite(header,8,1,output)!=1){error=9;goto done;}
 for(unsigned n=0;n<count;n++){
  Input r;Output result;memset(&result,0,sizeof(result));
  if(fread(&r,sizeof(r),1,input)!=1){error=10;break;}error=guard(&r,curves);if(error)break;
  unsigned char context[0x380],profile[0x5a5c],root[0x30],resource[0x20],voices[12][64];
  memset(context,0xa5,sizeof(context));memset(profile,0x69,sizeof(profile));memset(root,0x6b,sizeof(root));memset(resource,0x71,sizeof(resource));memset(voices,0x73,sizeof(voices));
  unsigned char *controller=context+32;memcpy(controller,r.controller,0x340);
  memcpy(profile,curves,8192);memcpy(profile+0x3a5c,curves+2048,8192);
  put(controller,0x118,(unsigned)(uintptr_t)profile);put(controller,0x128,(unsigned)(uintptr_t)root);
  put(root,0x2c,(unsigned)(uintptr_t)resource);put(resource,0x10,r.resource);
  for(unsigned i=0;i<12;i++)put(controller,0x1ec+i*4,(unsigned)(uintptr_t)voices[i]);
  uintptr_t group_vtable[6]={0},play_vtable[9]={0};
  group_vtable[5]=(uintptr_t)group_service;play_vtable[8]=(uintptr_t)play_service;
  uintptr_t group_object=(uintptr_t)group_vtable,play_object=(uintptr_t)play_vtable;
  put(image,0xe5025c-0x400000,(unsigned)(uintptr_t)&group_object);
  put(image,0xe50258-0x400000,(unsigned)(uintptr_t)&play_object);
  memcpy(image+0xe5a414-0x400000,&r.cooldown,4);
  unsigned char old_profile[sizeof(profile)],old_root[sizeof(root)],old_resource[sizeof(resource)],old_voices[sizeof(voices)];
  memcpy(old_profile,profile,sizeof(profile));memcpy(old_root,root,sizeof(root));memcpy(old_resource,resource,sizeof(resource));memcpy(old_voices,voices,sizeof(voices));
  active=&result;request=&r;active_controller=controller;bad_service=0;
  __try{((Restart)(image+0x423210-0x400000))(controller,NULL,r.category);}__except(fault(GetExceptionInformation())){error=20;}
  if(bad_service)error=21;
  if(memcmp(profile,old_profile,sizeof(profile))||memcmp(root,old_root,sizeof(root))||memcmp(resource,old_resource,sizeof(resource))||memcmp(voices,old_voices,sizeof(voices)))error=22;
  for(unsigned i=0;i<32;i++)if(context[i]!=0xa5||context[0x360+i]!=0xa5)error=23;
  if(get(controller,0x118)!=(unsigned)(uintptr_t)profile||get(controller,0x128)!=(unsigned)(uintptr_t)root)error=24;
  put(controller,0x118,get(r.controller,0x118));put(controller,0x128,get(r.controller,0x128));
  for(unsigned i=0;i<12;i++){if(get(controller,0x1ec+i*4)!=(unsigned)(uintptr_t)voices[i])error=24;put(controller,0x1ec+i*4,get(r.controller,0x1ec+i*4));}
  for(unsigned i=0;i<2;i++)memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);
  if(memcmp(image,saved,pe->OptionalHeader.SizeOfImage))error=25;
  if(error)break;
  result.index=r.index;memcpy(result.controller,controller,0x340);if(fwrite(&result,sizeof(result),1,output)!=1){error=26;break;}completed++;
 }
 if(!error&&fgetc(input)!=EOF)error=27;
done:
 if(input)fclose(input);if(output&&fclose(output))error=28;
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);DWORD old;if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&old))error=29;}
 unsigned unchanged=memcmp(image,saved,pe->OptionalHeader.SizeOfImage)==0;if(!unchanged)error=40;
 printf("engine_restart_cases=%u completed=%u entire_restored_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(saved);VirtualFree(image,0,MEM_RELEASE);return (int)error;
}
