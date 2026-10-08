/* Isolated whole 9d4970. Only three verified real kernel32 clock imports bind. */
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

enum { DESCRIPTOR=0xa500, SETTINGS=0x1050, RECORD=48, MAX_RECORDS=512 };
/* Dummy EDX keeps the three original arguments on the stack (RET12). */
typedef void (__fastcall *Kernel)(void *,void *,void *,int,float);
typedef struct { unsigned address; const char *name; DWORD original; FARPROC resolved; } Binding;
static Binding bindings[]={
 {0xc8911c,"QueryPerformanceFrequency",0,NULL},
 {0xc89218,"QueryPerformanceCounter",0,NULL},
 {0xc8925c,"GetTickCount",0,NULL}
};
static uintptr_t reference_base;
static int fault_details(EXCEPTION_POINTERS *p){
 fprintf(stderr,"fault=%08lx at_reference=%08lx access=%lu target_reference=%08lx\n",p->ExceptionRecord->ExceptionCode,
  (unsigned long)((uintptr_t)p->ExceptionRecord->ExceptionAddress-reference_base+0x400000),
  (unsigned long)p->ExceptionRecord->ExceptionInformation[0],
  (unsigned long)(p->ExceptionRecord->ExceptionInformation[1]-reference_base+0x400000));
 return EXCEPTION_EXECUTE_HANDLER;
}
static int clocks(unsigned char *image,int restore){
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 size_t end=pe->OptionalHeader.SizeOfImage;
 if(!restore){
  IMAGE_DATA_DIRECTORY dir=pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
  if(!inside(dir.VirtualAddress,dir.Size,end))return 0;
  unsigned found[3]={0};
  for(unsigned i=0;i<256;++i){
   size_t offset=dir.VirtualAddress+i*sizeof(IMAGE_IMPORT_DESCRIPTOR);
   if(!inside(offset,sizeof(IMAGE_IMPORT_DESCRIPTOR),dir.VirtualAddress+dir.Size))return 0;
   IMAGE_IMPORT_DESCRIPTOR *entry=(IMAGE_IMPORT_DESCRIPTOR *)(image+offset);
   if(!entry->Name)break;
   if(!inside(entry->Name,13,end)||memcmp(image+entry->Name,"KERNEL32.dll",13))continue;
   if(!entry->OriginalFirstThunk)return 0;
   for(unsigned j=0;j<4096;++j){
    if(!inside(entry->OriginalFirstThunk+j*4,4,end)||!inside(entry->FirstThunk+j*4,4,end))return 0;
    DWORD lookup=*(DWORD *)(image+entry->OriginalFirstThunk+j*4);
    if(!lookup)break;
    if(lookup&IMAGE_ORDINAL_FLAG32)continue;
    for(unsigned b=0;b<3;++b){
     size_t length=strlen(bindings[b].name)+1;
     if(inside(lookup+2,length,end)&&!memcmp(image+lookup+2,bindings[b].name,length)){
      if(++found[b]!=1||entry->FirstThunk+j*4!=bindings[b].address-0x400000)return 0;
      bindings[b].original=*(DWORD *)(image+entry->FirstThunk+j*4);
      bindings[b].resolved=GetProcAddress(GetModuleHandleW(L"kernel32.dll"),bindings[b].name);
      if(!bindings[b].resolved)return 0;
     }
    }
   }
  }
  for(unsigned b=0;b<3;++b)if(found[b]!=1)return 0;
 }
 DWORD old,ignored;
 if(!VirtualProtect(image+0xc89000-0x400000,0x1000,PAGE_READWRITE,&old)||old!=PAGE_READONLY)return 0;
 for(unsigned b=0;b<3;++b){
  *(DWORD *)(image+bindings[b].address-0x400000)=restore?bindings[b].original:(DWORD)(uintptr_t)bindings[b].resolved;
 }
 return VirtualProtect(image+0xc89000-0x400000,0x1000,old,&ignored)!=0;
}
static int finite_words(const unsigned char *data,unsigned count){
 for(unsigned i=0;i<count;++i){float value;memcpy(&value,data+i*4,4);if(!isfinite(value))return 0;}return 1;
}
static int valid(unsigned char *d,unsigned char *s,unsigned char *r,unsigned records,float gain,unsigned *g,unsigned clip){
 if(records>MAX_RECORDS||!isfinite(gain)||fabsf(gain)>100.f||clip>1)return 0;
 unsigned count=1;
 for(unsigned i=0;i<3;++i){int dim=*(int *)(d+0xa414+i*4);if(dim<2||dim>16)return 0;count*=(unsigned)dim;}
 if(count>400||*(int *)(d+0xa4a0)<0||*(int *)(d+0xa4a0)>2||*(int *)(d+0xa450)<0||*(int *)(d+0xa450)>64||d[0xa4f0]>1)return 0;
 float spacing=*(float *)(d+0xa420);
 if(!isfinite(spacing)||spacing<=0.f||!finite_words(s+0x1024,11)||!finite_words(d+0xa440,4))return 0;
 for(unsigned i=0;i<400;++i){unsigned char *n=d+0x2580+i*80;
  if(!finite_words(n,17))return 0;
  for(unsigned j=0;j<4;++j)if(*(float *)(n+0x20+j*4)<0.f)return 0;
 }
 int edges=*(int *)(d+0xa410);if(edges<0||edges>1200)return 0;
 for(int i=0;i<edges;++i){short *e=(short *)(d+i*8);
  if(e[0]<0||e[0]>=400||e[1]<0||e[1]>=400||!finite_words(d+i*8+4,1))return 0;
 }
 for(unsigned i=0;i<records;++i){unsigned char *p=r+i*RECORD;short index=*(short *)(p+0x20);
  if(index<0||index>=400||!finite_words(p,8)||!finite_words(p+0x24,1))return 0;
 }
 for(unsigned i=0;i<6;++i)if(i!=3&&!finite_words((unsigned char *)(g+i),1))return 0;
 return g[3]>=1&&g[3]<=64;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;reference_base=(uintptr_t)image;
 if(!clocks(image,0))return 20;
 const unsigned pages[]={0xe4f000,0xe50000,0xe58000,0xe8f000,0xec0000};
 DWORD protection[5],ignored;unsigned char initial[5][4096];
 for(unsigned i=0;i<5;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);
  if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 21;
 }
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 22;
 memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,total=0,magic=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||
  fread(&magic,4,1,input)!=1||magic!=0x314d4947||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;++c){
  __declspec(align(16)) unsigned char descriptor[DESCRIPTOR],settings[SETTINGS],records[MAX_RECORDS*RECORD];
  unsigned count,globals[6],clip;float gain;
  if(fread(&count,4,1,input)!=1||fread(&gain,4,1,input)!=1||count>MAX_RECORDS||
   fread(descriptor,1,DESCRIPTOR,input)!=DESCRIPTOR||fread(settings,1,SETTINGS,input)!=SETTINGS||
   fread(globals,4,6,input)!=6||fread(&clip,4,1,input)!=1||fread(records,RECORD,count,input)!=count){error=7;break;}
  if(!valid(descriptor,settings,records,count,gain,globals,clip)){error=8;break;}
  for(unsigned i=0;i<5;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xe4fc80-0x400000,globals,24);image[0xe500b8-0x400000]=(unsigned char)clip;
  *(void **)(descriptor+0xa4f8)=settings;
  Kernel kernel=(Kernel)(image+0x9d4970-0x400000);
  __try{kernel(descriptor,NULL,records,(int)count,gain);}__except(fault_details(GetExceptionInformation())){error=9;break;}
  *(unsigned *)(descriptor+0xa4f8)=0;
  for(unsigned i=0;i<400;++i)if(!finite_words(descriptor+0x2580+i*80,17))error=10;
  if(!finite_words(descriptor+0xa440,4))error=10;
  if(error)break;
  fwrite(descriptor,1,DESCRIPTOR,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){
  size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;
  if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=23;
 }
 for(unsigned i=0;i<5;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);
  if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=24;
 }
 if(!clocks(image,1))error=25;
 if(ferror(input)||ferror(output))error=12;
 int close_input=fclose(input),close_output=fclose(output);if(close_input||close_output)error=12;
 free(code);VirtualFree(image,0,MEM_RELEASE);
 printf("deformation_impact_cases=%u error=%u real_clock_imports=3 code_unchanged=%u\n",cases,error,error!=23);return (int)error;
}
