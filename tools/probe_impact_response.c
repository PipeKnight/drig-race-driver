/* Isolated original response slice. Own-thread hardware stop after the real
   prologue supplies solved records/state and resumes at9EB570. Contact generation,
   9D4210 and the solver are explicitly skipped, never claimed as replayed here. */
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
typedef void (__cdecl *Producer)(void *,void **,int,float,void *,void *,void *,void *);
static DWORD boundary,tail,epilogue,saved_dr[6];
static unsigned hit,faulted,record_count;
static unsigned char *descriptor,*linked_state;
static __declspec(align(16)) unsigned char records[400*48];
static LONG WINAPI debug_control(EXCEPTION_POINTERS *p){
 CONTEXT *c=p->ContextRecord;
 if(p->ExceptionRecord->ExceptionCode==0xe0424752){
  if(p->ExceptionRecord->ExceptionInformation[0]==0){
   saved_dr[0]=c->Dr0;saved_dr[1]=c->Dr1;saved_dr[2]=c->Dr2;saved_dr[3]=c->Dr3;saved_dr[4]=c->Dr6;saved_dr[5]=c->Dr7;
   c->Dr0=boundary;c->Dr1=c->Dr2=c->Dr3=c->Dr6=0;c->Dr7=1;
  }else{c->Dr0=saved_dr[0];c->Dr1=saved_dr[1];c->Dr2=saved_dr[2];c->Dr3=saved_dr[3];c->Dr6=saved_dr[4];c->Dr7=saved_dr[5];}
  return EXCEPTION_CONTINUE_EXECUTION;
 }
 if(p->ExceptionRecord->ExceptionCode==EXCEPTION_SINGLE_STEP&&c->Eip==boundary&&(c->Dr6&1)&&!hit){
  /* ESP is aligned after the original three nonvolatile-register pushes.
     The original response uses only these supplied locals and EDI/arguments. */
  *(unsigned *)(uintptr_t)(c->Esp+0x28)=record_count;
  memcpy((void *)(uintptr_t)(c->Esp+0x70),linked_state+0x360,16);
  memcpy((void *)(uintptr_t)(c->Esp+0x3f0),records,record_count*48);
  c->Edi=(DWORD)(uintptr_t)descriptor;c->Dr7=0;c->Dr6=0;
  c->Eip=record_count?tail:epilogue;hit=1;
  return EXCEPTION_CONTINUE_EXECUTION;
 }
 return EXCEPTION_CONTINUE_SEARCH;
}
static int fail(EXCEPTION_POINTERS *p){fprintf(stderr,"response_fault=%08lx ip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);faulted=1;return EXCEPTION_EXECUTE_HANDLER;}
static int finite_words(const unsigned char *p,unsigned count){for(unsigned i=0;i<count;++i){float v;memcpy(&v,p+i*4,4);if(!isfinite(v))return 0;}return 1;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 boundary=(DWORD)(uintptr_t)(image+0x9ea6f6-0x400000);tail=(DWORD)(uintptr_t)(image+0x9eb570-0x400000);epilogue=(DWORD)(uintptr_t)(image+0x9ebcbf-0x400000);
 const unsigned pages[]={0xe4f000,0xe50000,0xe58000};DWORD protection[3],ignored;unsigned char initial[3][4096];
 for(unsigned i=0;i<3;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 5;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 6;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 PVOID handler=AddVectoredExceptionHandler(1,debug_control);if(!handler)return 7;
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,total=0,magic=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 8;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31525049||fread(&total,4,1,input)!=1||!total||total>1024){error=9;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char d[0xa500]={0},linked[0x424]={0},contacts[80][0x80]={0};
  unsigned header[16];float response[80];void *pointers[80];
  if(fread(header,4,16,input)!=16){error=10;break;}
  unsigned nodes=header[0],count=header[1];record_count=header[2];
  if(!nodes||nodes>400||!count||count>80||record_count>400||header[3]>1||header[4]>1||header[5]>1||!finite_words((unsigned char *)(header+6),10)){error=11;break;}
  for(unsigned i=0;i<nodes;++i)if(fread(d+0x2580+i*80,1,16,input)!=16||!finite_words(d+0x2580+i*80,4))error=10;
  if(error)break;
  for(unsigned i=0;i<count;++i){unsigned char c[32];
   if(fread(c,1,32,input)!=32||*(unsigned *)c>1||*(unsigned *)(c+4)>1||!finite_words(c+8,6)){error=11;break;}
   contacts[i][0x73]=(unsigned char)*(unsigned *)c;contacts[i][0xc]=*(unsigned *)(c+4)?0:1;
   *(void **)(contacts[i]+8)=linked;memcpy(contacts[i]+0x38,c+8,8);memcpy(contacts[i]+0x60,c+16,16);pointers[i]=contacts[i];
  }
  if(error)break;
  memset(records,0,sizeof(records));
  for(unsigned i=0;i<record_count;++i){unsigned char r[40];
   if(fread(r,1,40,input)!=40||!finite_words(r,8)||!finite_words(r+36,1)){error=11;break;}
   unsigned node=*(unsigned short *)(r+32),contact=*(unsigned short *)(r+34);
   if(node>=nodes||contact>=count||!contacts[contact][0x73]||*(float *)(contacts[contact]+0x3c)<=0||
    (i&&*(unsigned short *)(records+(i-1)*48+0x22)>contact)){error=11;break;}
   memcpy(records+i*48,r,36);memcpy(records+i*48+0x28,r+36,4);
  }
  if(error)break;
  if(fread(response,4,count,input)!=count||!finite_words((unsigned char *)response,count)){error=10;break;}
  for(unsigned i=0;i<3;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xe4fc38-0x400000,header+6,4);memcpy(image+0xe500c0-0x400000,header+7,4);image[0xe500c4-0x400000]=(unsigned char)header[4];
  memcpy(linked+0x360,header+8,16);memcpy(linked+0x380,header+12,16);linked[0x390]=(unsigned char)header[5];
  descriptor=d;linked_state=linked;hit=faulted=0;ULONG_PTR action=0;RaiseException(0xe0424752,0,1,&action);
  Producer kernel=(Producer)(image+0x9ea6e0-0x400000);
  __try{kernel(linked,pointers,(int)count,0,header[3]?response:NULL,NULL,NULL,d);}__except(fail(GetExceptionInformation())){}
  action=1;RaiseException(0xe0424752,0,1,&action);
  if(!hit||faulted){error=12;break;}
  unsigned dirty=linked[0x390];fwrite(linked+0x380,1,16,output);fwrite(&dirty,4,1,output);fwrite(response,4,count,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=13;
finish:
 RemoveVectoredExceptionHandler(handler);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=14;}
 for(unsigned i=0;i<3;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=15;}
 if(ferror(input)||ferror(output))error=16;int ci=fclose(input),co=fclose(output);if(ci||co)error=16;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("impact_response_slice_cases=%u error=%u code_unchanged=%u supplied_context=9ea6f6_to_9eb570\n",cases,error,error!=14);return (int)error;
}
