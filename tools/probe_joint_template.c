/* Original constraint template stage, JTP1: the template's three frames
   0x7C7C00 (transform times the props quaternion matrices 0x7C06B0,
   0x7C08E0, 0x7C0B10), then the joint request 0x7C7B10 with 0x7C04D0 (props
   limits, springs and break impulses times the entity scale). Supplied props
   and template objects; no world. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include "isolated_reference.h"
__declspec(noinline) static void frames(void *function,void *template_){
 __asm { mov ecx,template_
         call function }
}
__declspec(noinline) static void request(void *function,void *template_,void *target,unsigned scale,unsigned select){
 __asm { mov ecx,template_
         push select
         push scale
         push target
         call function }
}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static unsigned char *base;
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"template_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,(unsigned long)(p->ContextRecord->Eip-(DWORD)(uintptr_t)base+0x400000),p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;base=image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 /* 0x7C06B0's lazy constant (0xE5AF20/0xE5AF30). */
 unsigned pages[1]={0xe5a000};DWORD protections[1]={0};unsigned char saved[1][4096];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<1;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,&protections[i])){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x3150544a||!read(input,&total,4)||!total||total>65536))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  __declspec(align(16)) unsigned char props[0x110],template_[0x150],made[0x180];
  unsigned h[2];
  if(!read(input,props,0x110)||!read(input,template_,0x150)||!read(input,h,8)||h[1]>1){error=8;break;}
  put(props,0xfc,0);
  put(template_,0x148,(unsigned)(uintptr_t)props);
  memset(made,0,sizeof made);
  __try{frames(image+0x7c7c00-0x400000,template_);request(image+0x7c7b10-0x400000,template_,made,h[0],h[1]);}__except(fault(GetExceptionInformation())){error=9;}
  if(error)break;
  /* The angular-extent callback as an image address. */
  {unsigned callback;memcpy(&callback,made+0x168,4);if(callback)put(made,0x168,callback-(unsigned)(uintptr_t)image+0x400000);}
  fwrite(template_+0x40,1,0xc0,output);fwrite(made,1,0x174,output);
  ++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;if(input)fclose(input);if(output)fclose(output);
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("template_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
