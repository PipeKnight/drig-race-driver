/* Whole original9EA6E0: generation/body/grid solver/response. Original CRT rand
   runs against a controlled private TLS seed, with seven real Windows imports.
   No original code patches, helper stubs or skipped instructions. */
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
#include "isolated_imports.h"
enum {DESCRIPTOR=0xa500,SETTINGS=0x1050,LINKED=0x424,CONTACT=0x80,MAX_CONTACTS=80};
typedef void (__cdecl *Kernel)(void *,void **,int,float,void *,void *,void *,void *);
#ifdef GRID_COLLISION_CALLBACK
typedef unsigned (__cdecl *Callback)(void *,void **,int,float,void *,void *,void *);
enum {BODY_SIZE=0xd230,PAGE_COUNT=10,MAGIC=0x31434349,MIN_CONTACTS=0};
#define PROBE_LABEL "collision_callback_whole"
#else
enum {BODY_SIZE=0x1650,PAGE_COUNT=9,MAGIC=0x31574349,MIN_CONTACTS=1};
#define PROBE_LABEL "impact_producer_whole"
#endif
static GridBinding bindings[]={ {0xc89148,"GetLastError",0,NULL},{0xc890d4,"SetLastError",0,NULL},
 {0xc8912c,"TlsGetValue",0,NULL},{0xc89130,"TlsSetValue",0,NULL},{0xc8911c,"QueryPerformanceFrequency",0,NULL},
 {0xc89218,"QueryPerformanceCounter",0,NULL},{0xc8925c,"GetTickCount",0,NULL} };
static int finite_words(const unsigned char *p,unsigned count){for(unsigned i=0;i<count;++i){float value;memcpy(&value,p+i*4,4);if(!isfinite(value))return 0;}return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"producer_fault=%08lx ip=%08lx access=%lu target=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,(unsigned long)p->ExceptionRecord->ExceptionInformation[0],(unsigned long)p->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;if(!grid_imports(image,bindings,7,0))return 5;
 DWORD ptd_slot=TlsAlloc(),cache_slot=TlsAlloc();if(ptd_slot==TLS_OUT_OF_INDEXES||cache_slot==TLS_OUT_OF_INDEXES)return 6;
 const unsigned pages[]={0xe4f000,0xe50000,0xe51000,0xe56000,0xe58000,0xe5a000,0xe85000,0xe8f000,0xec0000
#ifdef GRID_COLLISION_CALLBACK
 ,0xd16000
#endif
 };
 DWORD protection[PAGE_COUNT],ignored;unsigned char initial[PAGE_COUNT][4096];
 for(unsigned i=0;i<PAGE_COUNT;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 7;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 8;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,total=0,magic=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 9;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=MAGIC||fread(&total,4,1,input)!=1||!total||total>1024){error=10;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char d[DESCRIPTOR],s[SETTINGS],linked[LINKED],body[BODY_SIZE]={0},wrapper[16]={0};
  __declspec(align(16)) unsigned char contacts[MAX_CONTACTS][CONTACT],others[MAX_CONTACTS][0x370],center[16],a[16],b[16],ptd[0x214]={0};
  unsigned char globals[32],corner[4],mass[4],body_globals[20],scale[4],solver_globals[24];float response[MAX_CONTACTS],scalar;
  unsigned count,old_linked,seed,clip,present;void *pointers[MAX_CONTACTS];
  if(fread(&count,4,1,input)!=1||count<MIN_CONTACTS||count>MAX_CONTACTS||fread(&scalar,4,1,input)!=1||fread(&old_linked,4,1,input)!=1||
   fread(d,1,DESCRIPTOR,input)!=DESCRIPTOR||fread(s,1,SETTINGS,input)!=SETTINGS||fread(linked,1,LINKED,input)!=LINKED||
   fread(body+0x1630,1,16,input)!=16||fread(body+0xb20,1,16,input)!=16||fread(globals,1,32,input)!=32||fread(corner,1,4,input)!=4||fread(mass,1,4,input)!=4){error=11;break;}
  unsigned nodes=1;for(unsigned i=0;i<3;++i){int dim=*(int *)(d+0xa414+i*4);if(dim<2||dim>16){error=12;break;}nodes*=(unsigned)dim;}
  if(error||nodes>400||!isfinite(scalar)||!finite_words(linked+0x1d0,24)||!finite_words(linked+0x360,4)||!finite_words(linked+0x380,4)||linked[0x390]>1||
   !finite_words(body+0x1630,4)||!finite_words(body+0xb20,4)||!finite_words(d+0xa420,8)||*(float *)(d+0xa420)<=0||!finite_words(s+0x1024,11)||!finite_words(d+0xa4b4,13)||
   !finite_words(globals+8,4)||!finite_words(corner,1)||!finite_words(mass,1)){error=12;break;}
  for(unsigned i=0;i<400;++i)if(!finite_words(d+0x2580+i*80,17))error=12;
  int edges=*(int *)(d+0xa410);if(edges<0||edges>1200){error=12;break;}
  for(int i=0;i<edges;++i){short *e=(short *)(d+i*8);if(e[0]<0||e[0]>=400||e[1]<0||e[1]>=400||!finite_words(d+i*8+4,1))error=12;}
  if(error)break;
  for(unsigned i=0;i<count;++i){unsigned has_other;
   if(fread(contacts[i],1,CONTACT,input)!=CONTACT||fread(&has_other,4,1,input)!=1||has_other>1||fread(others[i]+0x360,1,16,input)!=16){error=11;break;}
   if(!finite_words(contacts[i]+0x38,4)||!finite_words(contacts[i]+0x50,8)||!finite_words(others[i]+0x360,4)){error=12;break;}
   unsigned reverse=contacts[i][0xc]==0&&*(unsigned *)(contacts[i]+8)==old_linked;if(reverse&&!has_other){error=12;break;}
   void *other=has_other?others[i]:NULL;if(reverse){*(void **)(contacts[i]+8)=linked;*(void **)(contacts[i]+4)=other;}
   else{*(void **)(contacts[i]+8)=other;*(void **)(contacts[i]+4)=linked;}pointers[i]=contacts[i];
  }
  if(error)break;
  if(fread(center,1,16,input)!=16||fread(body+0x290,1,40,input)!=40||fread(body+0x2e0,1,4,input)!=4||fread(body+0x15a8,1,4,input)!=4||
   fread(body_globals,1,20,input)!=20||fread(&seed,4,1,input)!=1||fread(a,1,16,input)!=16||fread(b,1,16,input)!=16||
   fread(scale,1,4,input)!=4||fread(solver_globals,1,24,input)!=24||fread(&clip,4,1,input)!=1||fread(&present,4,1,input)!=1||fread(response,4,count,input)!=count){error=11;break;}
  if(!finite_words(center,4)||!finite_words(body+0x290,10)||!finite_words(body+0x2e0,1)||!finite_words(body+0x15a8,1)||*(float *)(body+0x15a8)<=0||
   !finite_words(body_globals,5)||*(float *)(body_globals+8)<=0||!finite_words(a,4)||!finite_words(b,4)||!finite_words(scale,1)||!finite_words(solver_globals,6)||clip>1||present>1||!finite_words((unsigned char *)response,count)){error=12;break;}
  for(unsigned i=0;i<PAGE_COUNT;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xe500c0-0x400000,globals,32);memcpy(image+0xe4fc2c-0x400000,corner,4);memcpy(image+0xe51d40-0x400000,mass,4);
  memcpy(image+0xe500a4-0x400000,body_globals,20);memcpy(image+0xe4fc38-0x400000,scale,4);memcpy(image+0xe4fc80-0x400000,solver_globals,24);image[0xe500b8-0x400000]=(unsigned char)clip;
  *(DWORD *)(image+0xe56968-0x400000)=ptd_slot;*(DWORD *)(image+0xe5696c-0x400000)=cache_slot;*(unsigned *)(ptd+0x14)=seed;
  if(!TlsSetValue(ptd_slot,ptd)||!TlsSetValue(cache_slot,(void *)(uintptr_t)bindings[2].resolved)){error=13;break;}
  unsigned old_center=*(unsigned *)(linked+0x334);*(void **)(linked+0x334)=center;*(void **)(body+0xdb0)=wrapper;*(void **)(wrapper+8)=linked;*(void **)(d+0xa4f8)=s;
  *(void **)(image+0xe85f28-0x400000)=body;*(unsigned *)(image+0xe85fcc-0x400000)=1;
#ifdef GRID_COLLISION_CALLBACK
  unsigned char preparation[80];unsigned disabled,handled=0;
  if(fread(preparation,1,80,input)!=80||fread(&disabled,4,1,input)!=1){error=11;break;}
  if(disabled>1||d[0xa4af]>1||!finite_words(preparation,20)||*(float *)preparation<=0||*(float *)(preparation+4)<=0||*(float *)(preparation+8)<=0||!finite_words(linked+0x3f0,1)||*(float *)(linked+0x3f0)<=0||!finite_words(d+0xa494,1)){error=12;break;}
  memcpy(body+0xd16c,preparation,4);memcpy(body+0xd22c,preparation+4,4);memcpy(image+0xd1676c-0x400000,preparation+8,4);memcpy(image+0xe4fc30-0x400000,preparation+12,4);memcpy(image+0xe4fc48-0x400000,preparation+16,64);image[0xe85fad-0x400000]=(unsigned char)disabled;
  Callback callback=(Callback)(image+0xa064a0-0x400000);
  __try{handled=callback(linked,pointers,(int)count,scalar,NULL,present?response:NULL,d)&255;}__except(fault(GetExceptionInformation())){error=14;}
#else
  Kernel kernel=(Kernel)(image+0x9ea6e0-0x400000);
  __try{kernel(linked,pointers,(int)count,scalar,present?response:NULL,a,b,d);}__except(fault(GetExceptionInformation())){error=14;}
#endif
  if(error)break;
  *(unsigned *)(d+0xa4f8)=0;*(unsigned *)(linked+0x334)=old_center;
  fwrite(d,1,DESCRIPTOR,output);fwrite(linked,1,LINKED,output);fwrite(body+0x290,1,40,output);fwrite(body+0x2e0,1,4,output);fwrite(ptd+0x14,1,4,output);fwrite(response,4,count,output);++cases;
#ifdef GRID_COLLISION_CALLBACK
  fwrite(&handled,4,1,output);
#endif
 }
 if(!error&&fgetc(input)!=EOF)error=15;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=16;}
 for(unsigned i=0;i<PAGE_COUNT;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=17;}
 if(!grid_imports(image,bindings,7,1)||!TlsSetValue(ptd_slot,NULL)||!TlsSetValue(cache_slot,NULL)||!TlsFree(ptd_slot)||!TlsFree(cache_slot))error=18;
 if(ferror(input)||ferror(output))error=19;int ci=fclose(input),co=fclose(output);if(ci||co)error=19;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf(PROBE_LABEL "_cases=%u error=%u code_unchanged=%u real_imports=7 original_rand=1\n",cases,error,error!=16);return (int)error;
}
