/* Original close-drift flag post geometry, FPP1: the segment-against-box
   test 0x7BBB70 (thiscall post, box, half extents, out), the closest points
   of a segment and a box 0x7BC370 (stdcall a, b, box, half extents, out on
   the segment, out on the box) and the post push 0x7BCDA0 (thiscall post,
   box, half extents). Supplied post and box; no world, no bodies.
   Input: magic, count, then per case u32 mode (0, 1, 2), post 0x80, box
   matrix 0x40 (three axis rows, centre row), half extents 0x10, a 0x10,
   b 0x10. Output: magic, count, then per case the return word, two 0x10
   outputs and the post after the call. */
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
__declspec(noinline) static unsigned test(void *function,void *post,void *box,void *half,void *target){
 unsigned result;
 __asm { mov ecx,post
         push target
         push half
         push box
         call function
         movzx eax,al
         mov result,eax }
 return result;
}
__declspec(noinline) static void closest(void *function,void *a,void *b,void *box,void *half,void *on_segment,void *on_box){
 __asm { push on_box
         push on_segment
         push half
         push box
         push b
         push a
         call function }
}
__declspec(noinline) static void push_post(void *function,void *post,void *box,void *half){
 __asm { mov ecx,post
         push half
         push box
         call function }
}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static unsigned char *base;
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"post_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,(unsigned long)(p->ContextRecord->Eip-(DWORD)(uintptr_t)base+0x400000),p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;base=image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 /* The rsqrt/rcp refinement constants are lazy statics (0xE58B90..0xE58C30). */
 unsigned pages[1]={0xe58000};DWORD protections[1]={0};unsigned char saved[1][4096];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<1;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,&protections[i])){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31505046||!read(input,&total,4)||!total||total>65536))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  __declspec(align(16)) unsigned char post[0x80],box[0x40],half[0x10],a[0x10],b[0x10],first[0x10],second[0x10];
  unsigned mode,result=0;
  if(!read(input,&mode,4)||mode>2||!read(input,post,0x80)||!read(input,box,0x40)||!read(input,half,0x10)||!read(input,a,0x10)||!read(input,b,0x10)){error=8;break;}
  memset(first,0,sizeof first);memset(second,0,sizeof second);
  __try{
   if(mode==0)result=test(image+0x7bbb70-0x400000,post,box,half,first);
   else if(mode==1)closest(image+0x7bc370-0x400000,a,b,box,half,first,second);
   else push_post(image+0x7bcda0-0x400000,post,box,half);
  }__except(fault(GetExceptionInformation())){error=9;}
  if(error)break;
  fwrite(&result,4,1,output);fwrite(first,1,0x10,output);fwrite(second,1,0x10,output);fwrite(post,1,0x80,output);
  ++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;if(input)fclose(input);if(output)fclose(output);
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("post_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
