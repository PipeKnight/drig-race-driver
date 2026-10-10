/* Whole B7A810 + B791B0 and original memset; normal rows, no stubs/code writes. */
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
__declspec(noinline) static void normalize(void *function,void *manager,float relaxation){
 __asm {mov ecx,manager
        push relaxation
        call function}
}
__declspec(noinline) static void solve(void *function,void *manager,unsigned bodies,unsigned iterations){
 __asm {mov ecx,manager
        push iterations
        push bodies
        call function}
}
static int finite_row(const unsigned char *r){
 for(unsigned i=0;i<32;++i)if(!isfinite(*(const float *)(r+i*4)))return 0;
 const unsigned offsets[]={0x84,0x88,0x8c,0x90,0x94,0x98,0x9c,0xac,0xb0};
 for(unsigned i=0;i<sizeof(offsets)/sizeof(offsets[0]);++i)if(!isfinite(*(const float *)(r+offsets[i])))return 0;
 return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 /* Preserve the image's own memset dispatch; do not change its data/code. */
 unsigned dispatch=*(unsigned *)(image+0xfb1fa0-0x400000);
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31524e50||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;++c){
  unsigned h[5];float relaxation;__declspec(align(16)) unsigned char rows[256][192],manager[0xd0]={0};__declspec(align(16)) float bodies[64][8];
  if(fread(h,4,5,input)!=5||fread(&relaxation,4,1,input)!=1||h[0]>256||h[1]>h[0]||h[2]>64||h[3]>32||h[4]>1||!isfinite(relaxation)||fread(rows,192,h[0],input)!=h[0]||fread(bodies,32,h[2],input)!=h[2]){error=7;break;}
  for(unsigned i=0;i<h[2];++i)for(unsigned j=0;j<8;++j)if(!isfinite(bodies[i][j]))error=8;
  for(unsigned i=0;i<h[0];++i){if(!finite_row(rows[i])||*(float *)(rows[i]+0x88)>*(float *)(rows[i]+0x8c))error=8;
   if(h[4]){volatile float divisor=*(float *)(rows[i]+0xb0)+*(float *)(rows[i]+0x84),norm=*(float *)(rows[i]+0xac);if(divisor==0||norm==0){error=8;break;}volatile float factor=relaxation/divisor,coefficient=factor/norm;if(!isfinite(factor)||!isfinite(coefficient))error=8;}
  }
  for(unsigned group=0;group<2&&!error;++group){unsigned start=group?h[1]:0,end=group?h[0]:h[1];while(start<end){unsigned length=*(unsigned *)(rows[start]+0xb4),a=*(unsigned *)(rows[start]+0xa0),b=*(unsigned *)(rows[start]+0xa4);if(!length||length>end-start||a>=h[2]||(!group&&b>=h[2])){error=8;break;}for(unsigned j=start;j<start+length;++j)if(*(unsigned *)(rows[j]+0xa0)!=a||(!group&&*(unsigned *)(rows[j]+0xa4)!=b))error=8;start+=length;}}
  if(error)break;
  *(void **)(manager+0x80)=bodies;*(void **)(manager+0x8c)=rows;*(unsigned *)(manager+0xc4)=h[1];*(unsigned *)(manager+0xbc)=h[0];*(unsigned *)(manager+0xc8)=h[0];*(unsigned *)(manager+0xc0)=h[0];
  __try{if(h[4])normalize(image+0xb7a810-0x400000,manager,relaxation);solve(image+0xb791b0-0x400000,manager,h[2],h[3]);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"physical_response_fault=%08lx\n",GetExceptionCode());error=9;}
  if(error)break;for(unsigned i=0;i<h[0];++i)if(!finite_row(rows[i]))error=10;
  for(unsigned i=0;i<h[2];++i)for(unsigned j=0;j<8;++j)if(!isfinite(bodies[i][j]))error=10;
  if(error)break;fwrite(h,4,1,output);fwrite(h+2,4,1,output);fwrite(rows,192,h[0],output);fwrite(bodies,32,h[2],output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("physical_response_cases=%u error=%u code_unchanged=%u no_imports=1 original_memset_dispatch=%u\n",cases,error,error!=12,dispatch);return (int)error;
}
