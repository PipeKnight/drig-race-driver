/* Original 9EA6E0 prefix, stopped before 9D4210/solver/response via own-thread
   hardware execute breakpoint. This is deliberately NOT whole-producer replay. */
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

enum { DESCRIPTOR=0xa500, SETTINGS=0x1050, LINKED=0x424, CONTACT=0x80, MAX_CONTACTS=80 };
typedef void (__cdecl *Prefix)(void *,void **,int,float,void *,void *,void *,void *);
static DWORD boundary,saved_dr[6];
static unsigned captured_count,stopped,faulted;
static unsigned char generated[400*48];
static LONG WINAPI debug_control(EXCEPTION_POINTERS *p){
 if(p->ExceptionRecord->ExceptionCode!=0xe0424752)return EXCEPTION_CONTINUE_SEARCH;
 CONTEXT *c=p->ContextRecord;
 if(p->ExceptionRecord->ExceptionInformation[0]==0){
  saved_dr[0]=c->Dr0;saved_dr[1]=c->Dr1;saved_dr[2]=c->Dr2;
  saved_dr[3]=c->Dr3;saved_dr[4]=c->Dr6;saved_dr[5]=c->Dr7;
  c->Dr0=boundary;c->Dr1=c->Dr2=c->Dr3=c->Dr6=0;c->Dr7=1;
 }else{
  c->Dr0=saved_dr[0];c->Dr1=saved_dr[1];c->Dr2=saved_dr[2];
  c->Dr3=saved_dr[3];c->Dr6=saved_dr[4];c->Dr7=saved_dr[5];
 }
 return EXCEPTION_CONTINUE_EXECUTION;
}
static int prefix_stop(EXCEPTION_POINTERS *p){
 CONTEXT *c=p->ContextRecord;
 if(p->ExceptionRecord->ExceptionCode==EXCEPTION_SINGLE_STEP&&c->Eip==boundary&&(c->Dr6&1)){
  unsigned count=*(unsigned *)(uintptr_t)(c->Esp+0x28);
  if(count<=400){captured_count=count;memcpy(generated,(void *)(uintptr_t)(c->Esp+0x3f0),count*48);stopped=1;}
  else faulted=1;
 }else{
  fprintf(stderr,"prefix_fault=%08lx ip=%08lx\n",p->ExceptionRecord->ExceptionCode,c->Eip);faulted=1;
 }
 return EXCEPTION_EXECUTE_HANDLER;
}
static int finite_words(const unsigned char *p,unsigned count){
 for(unsigned i=0;i<count;++i){float value;memcpy(&value,p+i*4,4);if(!isfinite(value))return 0;}return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 boundary=(DWORD)(uintptr_t)(image+0x9eb534-0x400000);
 const unsigned pages[]={0xe4f000,0xe50000,0xe51000,0xe58000,0xe5a000,0xe85000};
 DWORD protection[6],ignored;unsigned char initial[6][4096];
 for(unsigned i=0;i<6;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);
  if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 5;
 }
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 6;
 memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 PVOID handler=AddVectoredExceptionHandler(1,debug_control);if(!handler)return 7;
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,total=0,magic=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 8;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||
  fread(&magic,4,1,input)!=1||magic!=0x31475049||fread(&total,4,1,input)!=1||!total||total>1024){error=9;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char d[DESCRIPTOR],s[SETTINGS],linked[LINKED],body[0x1650]={0},wrapper[16]={0};
  __declspec(align(16)) unsigned char contacts[MAX_CONTACTS][CONTACT],counterparts[MAX_CONTACTS][0x370];
  __declspec(align(16)) unsigned char prepared[32]={0};
  unsigned char globals[32],corner[4],mass[4];unsigned count,old_linked;float scalar;void *pointers[MAX_CONTACTS];
  if(fread(&count,4,1,input)!=1||!count||count>MAX_CONTACTS||fread(&scalar,4,1,input)!=1||fread(&old_linked,4,1,input)!=1||
   fread(d,1,DESCRIPTOR,input)!=DESCRIPTOR||fread(s,1,SETTINGS,input)!=SETTINGS||
   fread(linked,1,LINKED,input)!=LINKED||fread(body+0x1630,1,16,input)!=16||fread(body+0xb20,1,16,input)!=16||
   fread(globals,1,32,input)!=32||fread(corner,1,4,input)!=4||fread(mass,1,4,input)!=4){error=10;break;}
  unsigned nodes=1;for(unsigned i=0;i<3;++i){int dim=*(int *)(d+0xa414+i*4);if(dim<2||dim>16){error=11;break;}nodes*=(unsigned)dim;}
  if(error||nodes>400||!isfinite(scalar)||!finite_words(linked+0x1d0,16)||!finite_words(linked+0x360,4)||
   !finite_words(body+0x1630,4)||!finite_words(body+0xb20,4)||!finite_words(d+0xa420,8)||
   *(float *)(d+0xa420)<=0||!finite_words(s+0x1030,3)||!finite_words(d+0xa4b4,13)||
   !finite_words(globals+8,4)||!finite_words(corner,1)||!finite_words(mass,1)){error=11;break;}
  for(unsigned i=0;i<nodes;++i)if(!finite_words(d+0x2580+i*80,17))error=11;
  for(unsigned i=0;i<count;++i){unsigned has_counterpart;
   if(fread(contacts[i],1,CONTACT,input)!=CONTACT||fread(&has_counterpart,4,1,input)!=1||has_counterpart>1||
    fread(counterparts[i]+0x360,1,16,input)!=16){error=10;break;}
   if(!finite_words(contacts[i]+0x38,4)||!finite_words(contacts[i]+0x50,8)||!finite_words(counterparts[i]+0x360,4)){error=11;break;}
   unsigned reverse=contacts[i][0xc]==0&&*(unsigned *)(contacts[i]+8)==old_linked;
   if(reverse&&!has_counterpart){error=11;break;}
   void *other=has_counterpart?counterparts[i]:NULL;
   if(reverse){*(void **)(contacts[i]+8)=linked;*(void **)(contacts[i]+4)=other;}
   else{*(void **)(contacts[i]+8)=other;*(void **)(contacts[i]+4)=linked;}
   pointers[i]=contacts[i];
  }
  if(error)break;
  for(unsigned i=0;i<6;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xe500c0-0x400000,globals,32);memcpy(image+0xe4fc2c-0x400000,corner,4);memcpy(image+0xe51d40-0x400000,mass,4);
  *(void **)(body+0xdb0)=wrapper;*(void **)(wrapper+8)=linked;*(void **)(d+0xa4f8)=s;
  *(void **)(image+0xe85f28-0x400000)=body;*(unsigned *)(image+0xe85fcc-0x400000)=1;
  stopped=faulted=captured_count=0;memset(generated,0,sizeof(generated));
  ULONG_PTR action=0;RaiseException(0xe0424752,0,1,&action);
  Prefix kernel=(Prefix)(image+0x9ea6e0-0x400000);
  __try{kernel(linked,pointers,(int)count,scalar,NULL,prepared,prepared+16,d);}
  __except(prefix_stop(GetExceptionInformation())){}
  action=1;RaiseException(0xe0424752,0,1,&action);
  if(!stopped||faulted){error=12;break;}
  *(unsigned *)(d+0xa4f8)=0;
  fwrite(&captured_count,4,1,output);fwrite(d,1,DESCRIPTOR,output);fwrite(generated,48,captured_count,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=13;
finish:
 RemoveVectoredExceptionHandler(handler);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){
  size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;
  if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=14;
 }
 for(unsigned i=0;i<6;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);
  if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=15;
 }
 if(ferror(input)||ferror(output))error=16;
 int ci=fclose(input),co=fclose(output);if(ci||co)error=16;
 free(code);VirtualFree(image,0,MEM_RELEASE);
 printf("impact_generation_prefix_cases=%u error=%u code_unchanged=%u hardware_boundary=9eb534\n",cases,error,error!=14);return (int)error;
}
