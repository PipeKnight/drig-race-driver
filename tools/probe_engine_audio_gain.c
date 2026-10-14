/* Whole unchanged 42DE60 publication with owned voices and observed descriptors.
   Camera mode 3 only. No device backend, live injection or helper replacement. */
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
typedef void (__fastcall *Gain)(void *,void *);
#pragma pack(push,1)
typedef struct {unsigned index;unsigned char controller[0x340],voices[12][0x80];float motion[8],ranges[4],trims[2],treble;unsigned player;} Input;
typedef struct {unsigned index;unsigned char controller[0x340],voices[12][0x80];unsigned status;} Output;
#pragma pack(pop)
static unsigned get(const void *p,unsigned at){unsigned v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static float scalar(const void *p,unsigned at){float v;memcpy(&v,(const unsigned char *)p+at,4);return v;}
static void put(void *p,unsigned at,unsigned v){memcpy((unsigned char *)p+at,&v,4);}
static int private_file(const wchar_t *path){wchar_t root[32768],full[32768];DWORD a=GetFullPathNameW(L"local\\",32768,root,NULL),b=GetFullPathNameW(path,32768,full,NULL);return a&&a<32768&&b&&b<32768&&!_wcsnicmp(root,full,wcslen(root));}
static unsigned guard(const Input *r){
 const unsigned fields[]={0xcc,0x140,0x144,0x148,0x1a0,0x2f8,0x330,0xc8};
 for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++)if(!isfinite(scalar(r->controller,fields[i])))return 30;
 for(unsigned i=0;i<3;i++)if(fabsf(scalar(r->controller,0x140+i*4))>1.f)return 31;
 if(scalar(r->controller,0xcc)<0||scalar(r->controller,0xcc)>1||fabsf(scalar(r->controller,0x1a0))>1||fabsf(scalar(r->controller,0x330))>48||scalar(r->controller,0x2f8)<.5f||scalar(r->controller,0x2f8)>2||scalar(r->controller,0xc8)<0||scalar(r->controller,0xc8)>1||r->controller[0xd8]>1||r->player>1)return 31;
 for(unsigned i=0;i<12;i++){float pitch=scalar(r->controller,0x1a4+i*4);if(!isfinite(pitch)||pitch<.5f||pitch>2)return 32;}
 for(unsigned i=0;i<8;i++)if(!isfinite(r->motion[i])||fabsf(r->motion[i])>1000)return 33;
 float squared=(r->motion[0]*r->motion[0]+0.f)+(r->motion[2]*r->motion[2]+r->motion[3]*r->motion[3]);
 if(squared>0&&squared<FLT_MIN)return 33;
 for(unsigned i=0;i<2;i++)for(unsigned j=0;j<3;j++){float p=scalar(r->controller,0xe0+i*16+j*4);if(!isfinite(p)||fabsf(p)>1000000)return 33;}
 for(unsigned i=0;i<4;i++)if(!isfinite(r->ranges[i])||r->ranges[i]<=0||r->ranges[i]>100000)return 34;
 if(r->ranges[0]>r->ranges[1]||r->ranges[2]>r->ranges[3])return 34;
 for(unsigned i=0;i<2;i++)if(!isfinite(r->trims[i])||fabsf(r->trims[i])>48)return 35;
 if(!isfinite(r->treble)||fabsf(r->treble)>96)return 35;
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
 const unsigned pages[]={0xe58000,0xe9c000};DWORD protections[2];unsigned protected=0,error=0,count=0,completed=0;
 for(unsigned i=0;i<2;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=5;break;}protected++;}
 FILE *input=NULL,*output=NULL;unsigned header[2];
 if(error||_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto done;}
 if(fread(header,8,1,input)!=1||header[0]!=0x31304745||!header[1]||header[1]>4096){error=7;goto done;}count=header[1];
 HANDLE file=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
 if(file==INVALID_HANDLE_VALUE){error=8;goto done;}
 int descriptor=_open_osfhandle((intptr_t)file,_O_BINARY);if(descriptor<0){CloseHandle(file);error=8;goto done;}
 output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);error=8;goto done;}
 if(fwrite(header,8,1,output)!=1){error=9;goto done;}
 for(unsigned n=0;n<count;n++){
  Input r;Output result;
  if(fread(&r,sizeof(r),1,input)!=1){error=10;break;}error=guard(&r);if(error)break;
  __declspec(align(16)) unsigned char context[0x380],voices[12][0xc0],root[0xa6c0],component[0x80],camera_wrapper[0x30],camera[0xc20],vehicle[0xdc0],physical_wrapper[0x10],physical[0x240],emitters[2][0x200],profiles[2][0x40],resource[0xe0];
  memset(context,0xa5,sizeof(context));memset(voices,0xa5,sizeof(voices));memset(root,0x6b,sizeof(root));memset(component,0x6b,sizeof(component));memset(camera_wrapper,0x6b,sizeof(camera_wrapper));memset(camera,0x6b,sizeof(camera));memset(vehicle,0x6b,sizeof(vehicle));memset(physical_wrapper,0x6b,sizeof(physical_wrapper));memset(physical,0x6b,sizeof(physical));memset(emitters,0x6b,sizeof(emitters));memset(profiles,0x6b,sizeof(profiles));memset(resource,0x6b,sizeof(resource));
  unsigned char *controller=context+32;memcpy(controller,r.controller,0x340);
  const unsigned offsets[]={0x110,0x114,0x128,0x12c,0x130};
  void *pointers[]={component,camera_wrapper,root,emitters[0],emitters[1]};
  for(unsigned i=0;i<5;i++)put(controller,offsets[i],(unsigned)(uintptr_t)pointers[i]);
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
  unsigned char owned_before[sizeof(root)+sizeof(component)+sizeof(camera_wrapper)+sizeof(camera)+sizeof(vehicle)+sizeof(physical_wrapper)+sizeof(physical)+sizeof(emitters)+sizeof(profiles)+sizeof(resource)+sizeof(params)];
  unsigned char *blocks[]={root,component,camera_wrapper,camera,vehicle,physical_wrapper,physical,(unsigned char *)emitters,(unsigned char *)profiles,resource,(unsigned char *)params};
  const unsigned sizes[]={sizeof(root),sizeof(component),sizeof(camera_wrapper),sizeof(camera),sizeof(vehicle),sizeof(physical_wrapper),sizeof(physical),sizeof(emitters),sizeof(profiles),sizeof(resource),sizeof(params)};
  unsigned cursor=0;for(unsigned i=0;i<11;i++){memcpy(owned_before+cursor,blocks[i],sizes[i]);cursor+=sizes[i];}
  __try{((Gain)(image+0x42de60-0x400000))(controller,NULL);}__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  cursor=0;for(unsigned i=0;i<11;i++){if(memcmp(owned_before+cursor,blocks[i],sizes[i]))error=21;cursor+=sizes[i];}
  for(unsigned i=0;i<32;i++)if(context[i]!=0xa5||context[0x360+i]!=0xa5)error=22;
  for(unsigned i=0;i<5;i++){if(get(controller,offsets[i])!=(unsigned)(uintptr_t)pointers[i])error=23;put(controller,offsets[i],get(r.controller,offsets[i]));}
  for(unsigned i=0;i<12;i++){
   if(get(controller,0x1ec+i*4)!=(unsigned)(uintptr_t)(voices[i]+32))error=23;
   put(controller,0x1ec+i*4,get(r.controller,0x1ec+i*4));
   for(unsigned j=0;j<32;j++)if(voices[i][j]!=0xa5||voices[i][0xa0+j]!=0xa5)error=22;
   memcpy(result.voices[i],voices[i]+32,0x80);
  }
  result.status=get(image,0xe9c14c-0x400000);if(result.status)error=24;
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
 printf("engine_gain_cases=%u completed=%u entire_restored_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(saved);VirtualFree(image,0,MEM_RELEASE);return (int)error;
}
