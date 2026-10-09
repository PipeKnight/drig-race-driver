/* Whole originalA06050 including original lookup/interpolation/mode1 settling.
   No imports, stubs, instruction skips or original executable writes. */
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
typedef float (__cdecl *Prepare)(void *,void *,void *,void *,void *);
static int finite_words(const unsigned char *p,unsigned count){for(unsigned i=0;i<count;++i){float v;memcpy(&v,p+i*4,4);if(!isfinite(v))return 0;}return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"preparation_fault=%08lx ip=%08lx access=%lu target=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,(unsigned long)p->ExceptionRecord->ExceptionInformation[0],(unsigned long)p->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 const unsigned pages[]={0xd16000,0xe4f000,0xe58000,0xe85000};DWORD protection[4],ignored;unsigned char initial[4][4096];
 for(unsigned i=0;i<4;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 5;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 6;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0,activated=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 7;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31504349||fread(&total,4,1,input)!=1||!total||total>1024){error=8;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char d[0xa500],s[0x1050],linked[0x424],body[0xd230]={0},wrapper[16]={0};
  __declspec(align(16)) unsigned char direction[16],inverse[64],point[16];unsigned char global[4],scale[4],table[64];float scalar=0;
  if(fread(d,1,sizeof(d),input)!=sizeof(d)||fread(s,1,sizeof(s),input)!=sizeof(s)||fread(linked,1,sizeof(linked),input)!=sizeof(linked)||
   fread(direction,1,16,input)!=16||fread(inverse,1,64,input)!=64||fread(point,1,16,input)!=16||fread(body+0xd16c,1,4,input)!=4||fread(body+0xd22c,1,4,input)!=4||
   fread(global,1,4,input)!=4||fread(scale,1,4,input)!=4||fread(table,1,64,input)!=64){error=9;break;}
  unsigned nodes=1;for(unsigned i=0;i<3;++i){int dim=*(int *)(d+0xa414+i*4);if(dim<2||dim>16){error=10;break;}nodes*=(unsigned)dim;}
  int edges=*(int *)(d+0xa410);unsigned mode=*(unsigned *)(d+0xa4a0);
  if(error||nodes>400||edges<0||edges>1200||mode>2||d[0xa4af]>1||!finite_words(d+0xa420,1)||*(float *)(d+0xa420)<=0||!finite_words(d+0xa430,4)||
   !finite_words(d+0xa494,1)||!finite_words(s+0x1024,2)||!finite_words(linked+0x1d0,16)||!finite_words(linked+0x3f0,1)||*(float *)(linked+0x3f0)<=0||
   !finite_words(direction,4)||!finite_words(inverse,16)||!finite_words(point,4)||!finite_words(body+0xd16c,1)||*(float *)(body+0xd16c)<=0||
   !finite_words(body+0xd22c,1)||*(float *)(body+0xd22c)<=0||!finite_words(global,1)||*(float *)global<=0||!finite_words(scale,1)||!finite_words(table,16)){error=10;break;}
  for(unsigned i=0;i<400;++i){unsigned char *n=d+0x2580+i*80;if(!finite_words(n,12))error=10;for(unsigned j=0;j<4;++j)if(*(float *)(n+32+j*4)<0)error=10;}
  for(int i=0;i<edges;++i){short *e=(short *)(d+i*8);if(e[0]<0||e[0]>=400||e[1]<0||e[1]>=400||!finite_words(d+i*8+4,1))error=10;}if(error)break;
  for(unsigned i=0;i<4;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xd1676c-0x400000,global,4);memcpy(image+0xe4fc30-0x400000,scale,4);memcpy(image+0xe4fc48-0x400000,table,64);
  *(void **)(body+0xdb0)=wrapper;*(void **)(wrapper+8)=linked;*(void **)(d+0xa4f8)=s;*(void **)(image+0xe85f28-0x400000)=body;*(unsigned *)(image+0xe85fcc-0x400000)=1;
  Prepare prepare=(Prepare)(image+0xa06050-0x400000);
  __try{scalar=prepare(d,linked,direction,inverse,point);}__except(fault(GetExceptionInformation())){error=11;}if(error)break;
  if(!isfinite(scalar)){error=12;break;}if(mode!=1&&*(unsigned *)(d+0xa4a0)==1)++activated;
  *(unsigned *)(d+0xa4f8)=0;fwrite(d,1,sizeof(d),output);fwrite(linked,1,sizeof(linked),output);fwrite(direction,1,16,output);fwrite(inverse,1,64,output);fwrite(point,1,16,output);fwrite(&scalar,4,1,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=13;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=14;}
 for(unsigned i=0;i<4;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=15;}
 if(ferror(input)||ferror(output))error=16;int ci=fclose(input),co=fclose(output);if(ci||co)error=16;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("contact_preparation_cases=%u activated=%u error=%u code_unchanged=%u no_imports=1\n",cases,activated,error,error!=14);return (int)error;
}
