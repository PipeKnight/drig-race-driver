/* Whole unchanged 4371C0 with existing owned voices and supplied playback service.
   Camera mode 3 only. Explicit virtual service boundary, no device or live injection. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <float.h>
#include <math.h>
#include <io.h>
#include <fcntl.h>
#include "isolated_reference.h"
typedef void (__fastcall *Schedule)(void *,void *,const double *);
#pragma pack(push,1)
typedef struct {unsigned index;unsigned char controller[0x340],voices[12][0x80];float motion[8],ranges[4],trims[2],treble;unsigned player;double delta;float cooldown;} Input;
typedef struct {unsigned slot,resource,sample,flags,group,requested,accepted;} Command;
typedef struct {unsigned index;unsigned char controller[0x340],voices[12][0x80];unsigned status,count;Command commands[6];} Output;
#pragma pack(pop)
static unsigned get(const void *p,unsigned at){unsigned v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static float scalar(const void *p,unsigned at){float v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static void put(void *p,unsigned at,unsigned v){memcpy((unsigned char *)p+at,&v,4);}
static int private_file(const wchar_t *path){wchar_t root[32768],full[32768];DWORD a=GetFullPathNameW(L"local\\",32768,root,NULL),b=GetFullPathNameW(path,32768,full,NULL);return a&&a<32768&&b&&b<32768&&!_wcsnicmp(root,full,wcslen(root));}
static Output *active;
static unsigned char *active_controller;
static unsigned bad_service;
static unsigned __fastcall group_service(void *self,void *unused,unsigned group){(void)self;(void)unused;return group^0x100;}
static void __fastcall play_service(void *self,void *unused,void **voice,unsigned resource,unsigned sample,unsigned flags,unsigned group,unsigned *seek){
 (void)self;(void)unused;
 uintptr_t start=(uintptr_t)(active_controller+0x1ec),at=(uintptr_t)voice;
 if(active->count>=6||at<start||at>=start+48||(at-start)%4){bad_service=1;return;}
 Command *c=active->commands+active->count++;
 c->slot=(unsigned)((at-start)/4);c->resource=resource;c->sample=sample;c->flags=flags;c->group=group;c->requested=*seek;c->accepted=*seek;
}
static unsigned guard(const Input *r,const float *curves){
 if(!isfinite(r->cooldown)||r->cooldown<0||r->cooldown>1)return 39;
 if(!isfinite(r->delta)||r->delta<0||r->delta>1)return 36;
 for(unsigned i=0;i<3;i++){float c=scalar(r->controller,0x134+i*4);if(!isfinite(c)||fabsf(c)>60||r->controller[0x14c+i]>1||r->controller[0x14f+i]>1)return 37;}
 if(!isfinite(scalar(r->controller,0x30c))||fabsf(scalar(r->controller,0x30c))>1)return 38;
 for(unsigned i=0;i<12;i++){float p=scalar(r->controller,0x230+i*4);unsigned l=get(r->controller,0x2c0+i*4);if(!isfinite(p)||fabsf(p)>1e9f||!l||l>10000000)return 38;}
 const unsigned fields[]={0xcc,0x140,0x144,0x148,0x1a0,0x2f8,0x330,0xc8};
 for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++)if(!isfinite(scalar(r->controller,fields[i])))return 30;
 for(unsigned i=0;i<3;i++)if(fabsf(scalar(r->controller,0x140+i*4))>1.f)return 31;
 if(scalar(r->controller,0xcc)<0||scalar(r->controller,0xcc)>1||fabsf(scalar(r->controller,0x1a0))>1||fabsf(scalar(r->controller,0x330))>48||scalar(r->controller,0x2f8)<.5f||scalar(r->controller,0x2f8)>2||scalar(r->controller,0xc8)<0||scalar(r->controller,0xc8)>1||r->controller[0xd8]>1||r->player>1)return 31;
 for(unsigned i=0;i<8;i++)if(!isfinite(r->motion[i])||fabsf(r->motion[i])>1000)return 33;
 float squared=(r->motion[0]*r->motion[0]+0.f)+(r->motion[2]*r->motion[2]+r->motion[3]*r->motion[3]);
 if(squared>0&&squared<FLT_MIN)return 33;
 for(unsigned i=0;i<2;i++)for(unsigned j=0;j<3;j++){float p=scalar(r->controller,0xe0+i*16+j*4);if(!isfinite(p)||fabsf(p)>1000000)return 33;}
 for(unsigned i=0;i<4;i++)if(!isfinite(r->ranges[i])||r->ranges[i]<=0||r->ranges[i]>100000)return 34;
 if(r->ranges[0]>r->ranges[1]||r->ranges[2]>r->ranges[3])return 34;
 for(unsigned i=0;i<2;i++)if(!isfinite(r->trims[i])||fabsf(r->trims[i])>48)return 35;
 if(!isfinite(r->treble)||fabsf(r->treble)>96)return 35;
 /* Same length/rate for each engine/exhaust and both pair banks, bounded PCM. */
 float target=scalar(r->controller,0xc8);
 for(unsigned c=0;c<3;c++){
  unsigned rate=get(r->controller,0x290+c*4),length=get(r->controller,0x2c0+c*4);
  if(rate<8000||rate>192000||!length||length>10000000)return 38;
  for(unsigned b=1;b<4;b++)if(get(r->controller,0x290+(b*3+c)*4)!=rate||get(r->controller,0x2c0+(b*3+c)*4)!=length)return 38;
  unsigned base=c?2048:0;float minimum=curves[c?2048:1023]*.5f;if(target<minimum)target=minimum;
  unsigned idx=(unsigned)(target*1024.f);if(idx>1023)idx=1023;
  unsigned seek=(unsigned)(((double)length*(double)curves[base+1024+idx]/(double)rate)*1000000.);
  float position=(float)(((double)seek*(double)1e-6f)*(double)rate);
  float time=(float)((double)(int)position/(double)length);if(time>1)time=1;
  idx=(unsigned)(time*1024.f);if(idx>1023)idx=1023;
  if((double)target/(double)curves[base+idx]>1.5)return 42;
 }
 return 0;
}
static unsigned fault(EXCEPTION_POINTERS *p){fprintf(stderr,"exception=%08lx instruction=%p",p->ExceptionRecord->ExceptionCode,p->ExceptionRecord->ExceptionAddress);if(p->ExceptionRecord->NumberParameters>=2)fprintf(stderr," address=%p",(void *)p->ExceptionRecord->ExceptionInformation[1]);fputc('\n',stderr);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
 if(argc!=4||!private_file(argv[1])||!private_file(argv[2])||!private_file(argv[3]))return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
 const unsigned pages[]={0xe50000,0xe58000,0xe5a000,0xe9c000};DWORD protections[4];unsigned protected=0,error=0,count=0,completed=0;
 for(unsigned i=0;i<4;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=5;break;}protected++;}
 FILE *input=NULL,*output=NULL;unsigned header[2];float curves[4096];
 if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(header,8,1,input)!=1||header[0]!=0x31305045||!header[1]||header[1]>4096){error=7;goto done;}count=header[1];
 if(fread(curves,sizeof(curves),1,input)!=1){error=7;goto done;}
 for(unsigned i=0;i<4096;i++)if(!isfinite(curves[i])||curves[i]<0||curves[i]>1||(i%2048<1024&&curves[i]<=0)){error=7;goto done;}
 HANDLE file=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
 if(file==INVALID_HANDLE_VALUE){error=8;goto done;}
 int descriptor=_open_osfhandle((intptr_t)file,_O_BINARY);if(descriptor<0){CloseHandle(file);error=8;goto done;}
 output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);error=8;goto done;}
 if(fwrite(header,8,1,output)!=1){error=9;goto done;}
 for(unsigned n=0;n<count;n++){
  Input r;Output result;memset(&result,0,sizeof(result));
  if(fread(&r,sizeof(r),1,input)!=1){error=10;break;}error=guard(&r,curves);if(error)break;
  __declspec(align(16)) unsigned char context[0x380],voices[12][0xc0],root[0xa6c0],component[0x80],camera_wrapper[0x30],camera[0xc20],vehicle[0xdc0],physical_wrapper[0x10],physical[0x240],emitters[2][0x200],profiles[2][0x40],resource[0xe0],curve_profile[0x5a5c];
  memset(context,0xa5,sizeof(context));memset(voices,0xa5,sizeof(voices));memset(root,0x6b,sizeof(root));memset(component,0x6b,sizeof(component));memset(camera_wrapper,0x6b,sizeof(camera_wrapper));memset(camera,0x6b,sizeof(camera));memset(vehicle,0x6b,sizeof(vehicle));memset(physical_wrapper,0x6b,sizeof(physical_wrapper));memset(physical,0x6b,sizeof(physical));memset(emitters,0x6b,sizeof(emitters));memset(profiles,0x6b,sizeof(profiles));memset(resource,0x6b,sizeof(resource));
  memset(curve_profile,0x69,sizeof(curve_profile));memcpy(curve_profile,curves,8192);memcpy(curve_profile+0x3a5c,curves+2048,8192);
  unsigned char *controller=context+32;memcpy(controller,r.controller,0x340);
  const unsigned offsets[]={0x110,0x114,0x128,0x12c,0x130,0x118};
  void *pointers[]={component,camera_wrapper,root,emitters[0],emitters[1],curve_profile};
  for(unsigned i=0;i<6;i++)put(controller,offsets[i],(unsigned)(uintptr_t)pointers[i]);
  put(resource,0x10,1);
  put(root,0x2c,(unsigned)(uintptr_t)resource);memcpy(root+0xa6a4,&r.treble,4);
  memcpy(resource+0xcc,r.trims,8);resource[0xd4]=0;
  put(component,0x24,(unsigned)(uintptr_t)vehicle);put(component,0x6c,r.player?0:1);
  put(camera_wrapper,0x28,(unsigned)(uintptr_t)camera);put(camera,0xc1c,3);
  put(vehicle,0xdb0,(unsigned)(uintptr_t)physical_wrapper);put(physical_wrapper,8,(unsigned)(uintptr_t)physical);memcpy(physical+0x210,r.motion,32);
  for(unsigned i=0;i<2;i++){put(emitters[i],0x1e0,(unsigned)(uintptr_t)profiles[i]);memcpy(profiles[i]+0x28,r.ranges+i*2,8);}
  for(unsigned i=0;i<12;i++){memcpy(voices[i]+32,r.voices[i],0x80);put(controller,0x1ec+i*4,(unsigned)(uintptr_t)(voices[i]+32));}
  unsigned params[8][7]={{0,0x58,0,0,0,0,1},{0,0x18,0,0,0,0,1},{0,0x14,0,0,0,0,1},{0,0x5c,0,0,0,0,1},{0,0x2c,0,0,0,4,1},{0,0,0,0,0,0,2},{0,0,0,0,0,0,2},{0,0x1c,0,0,0,4,1}};
  params[5][1]=(unsigned)(uintptr_t)(image+0xa23520-0x400000);params[6][1]=(unsigned)(uintptr_t)(image+0xa23510-0x400000);
  const unsigned globals[]={0xe9c180,0xe9c18c,0xe9c190,0xe9c19c,0xe9c1a0,0xe9c1a4,0xe9c1a8,0xe9c1b0};
  for(unsigned i=0;i<8;i++)put(image,globals[i]-0x400000,(unsigned)(uintptr_t)params[i]);
  unsigned char owned_before[sizeof(root)+sizeof(component)+sizeof(camera_wrapper)+sizeof(camera)+sizeof(vehicle)+sizeof(physical_wrapper)+sizeof(physical)+sizeof(emitters)+sizeof(profiles)+sizeof(resource)+sizeof(params)+sizeof(curve_profile)];
  unsigned char *blocks[]={root,component,camera_wrapper,camera,vehicle,physical_wrapper,physical,(unsigned char *)emitters,(unsigned char *)profiles,resource,(unsigned char *)params,curve_profile};
  const unsigned sizes[]={sizeof(root),sizeof(component),sizeof(camera_wrapper),sizeof(camera),sizeof(vehicle),sizeof(physical_wrapper),sizeof(physical),sizeof(emitters),sizeof(profiles),sizeof(resource),sizeof(params),sizeof(curve_profile)};
  unsigned cursor=0;for(unsigned i=0;i<12;i++){memcpy(owned_before+cursor,blocks[i],sizes[i]);cursor+=sizes[i];}
  uintptr_t group_vtable[6]={0},play_vtable[9]={0};
  group_vtable[5]=(uintptr_t)group_service;play_vtable[8]=(uintptr_t)play_service;
  uintptr_t group_object=(uintptr_t)group_vtable,play_object=(uintptr_t)play_vtable;
  put(image,0xe5025c-0x400000,(unsigned)(uintptr_t)&group_object);put(image,0xe50258-0x400000,(unsigned)(uintptr_t)&play_object);
  memcpy(image+0xe5a414-0x400000,&r.cooldown,4);active=&result;active_controller=controller;bad_service=0;
  __try{((Schedule)(image+0x4371c0-0x400000))(controller,NULL,&r.delta);}__except(fault(GetExceptionInformation())){error=20;}
  if(bad_service)error=41;
  if(error)break;
  cursor=0;for(unsigned i=0;i<12;i++){if(memcmp(owned_before+cursor,blocks[i],sizes[i]))error=21;cursor+=sizes[i];}
  for(unsigned i=0;i<32;i++)if(context[i]!=0xa5||context[0x360+i]!=0xa5)error=22;
  for(unsigned i=0;i<6;i++){if(get(controller,offsets[i])!=(unsigned)(uintptr_t)pointers[i])error=23;put(controller,offsets[i],get(r.controller,offsets[i]));}
  for(unsigned i=0;i<12;i++){
   if(get(controller,0x1ec+i*4)!=(unsigned)(uintptr_t)(voices[i]+32))error=23;
   put(controller,0x1ec+i*4,get(r.controller,0x1ec+i*4));
   for(unsigned j=0;j<32;j++)if(voices[i][j]!=0xa5||voices[i][0xa0+j]!=0xa5)error=22;
   memcpy(result.voices[i],voices[i]+32,0x80);
  }
  result.status=get(image,0xe9c14c-0x400000);if(result.status)error=24;
  for(unsigned i=0;i<4;i++)memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);
  if(memcmp(image,saved,pe->OptionalHeader.SizeOfImage))error=25;
  if(error)break;
  result.index=r.index;memcpy(result.controller,controller,0x340);if(fwrite(&result,sizeof(result),1,output)!=1){error=26;break;}completed++;
 }
 if(!error&&fgetc(input)!=EOF)error=27;
done:
 if(input)fclose(input);if(output&&fclose(output))error=28;
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);DWORD old;if(!VirtualProtect(image+pages[i]-0x400000,4096,protections[i],&old))error=29;}
 unsigned unchanged=memcmp(image,saved,pe->OptionalHeader.SizeOfImage)==0;if(!unchanged)error=40;
 printf("engine_schedule_playback_cases=%u completed=%u entire_restored_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(saved);VirtualFree(image,0,MEM_RELEASE);return (int)error;
}
