/* Whole original9D4210, including original CRT rand and real TLS APIs.
   Thread-data state is supplied in private TLS slots; no code stubs/patches. */
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
typedef void (__cdecl *Kernel)(void *,void *,void *,void *);
typedef struct {unsigned address;const char *name;DWORD original;FARPROC resolved;} Binding;
static Binding bindings[]={ {0xc89148,"GetLastError",0,NULL},{0xc890d4,"SetLastError",0,NULL},
 {0xc8912c,"TlsGetValue",0,NULL},{0xc89130,"TlsSetValue",0,NULL} };
static int bind(unsigned char *image,int restore){
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);size_t end=pe->OptionalHeader.SizeOfImage;
 if(!restore){
  IMAGE_DATA_DIRECTORY dir=pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];unsigned found[4]={0};
  if(!inside(dir.VirtualAddress,dir.Size,end))return 0;
  for(unsigned i=0;i<256;++i){size_t offset=dir.VirtualAddress+i*sizeof(IMAGE_IMPORT_DESCRIPTOR);
   if(!inside(offset,sizeof(IMAGE_IMPORT_DESCRIPTOR),dir.VirtualAddress+dir.Size))return 0;
   IMAGE_IMPORT_DESCRIPTOR *entry=(IMAGE_IMPORT_DESCRIPTOR *)(image+offset);if(!entry->Name)break;
   if(!inside(entry->Name,13,end)||memcmp(image+entry->Name,"KERNEL32.dll",13))continue;if(!entry->OriginalFirstThunk)return 0;
   for(unsigned j=0;j<4096;++j){
    if(!inside(entry->OriginalFirstThunk+j*4,4,end)||!inside(entry->FirstThunk+j*4,4,end))return 0;
    DWORD lookup=*(DWORD *)(image+entry->OriginalFirstThunk+j*4);if(!lookup)break;if(lookup&IMAGE_ORDINAL_FLAG32)continue;
    for(unsigned b=0;b<4;++b){size_t length=strlen(bindings[b].name)+1;
     if(inside(lookup+2,length,end)&&!memcmp(image+lookup+2,bindings[b].name,length)){
      if(++found[b]!=1||entry->FirstThunk+j*4!=bindings[b].address-0x400000)return 0;
      bindings[b].original=*(DWORD *)(image+entry->FirstThunk+j*4);bindings[b].resolved=GetProcAddress(GetModuleHandleW(L"kernel32.dll"),bindings[b].name);if(!bindings[b].resolved)return 0;
     }
    }
   }
  }
  for(unsigned b=0;b<4;++b)if(found[b]!=1)return 0;
 }
 DWORD old,ignored;if(!VirtualProtect(image+0xc89000-0x400000,4096,PAGE_READWRITE,&old)||old!=PAGE_READONLY)return 0;
 for(unsigned b=0;b<4;++b)*(DWORD *)(image+bindings[b].address-0x400000)=restore?bindings[b].original:(DWORD)(uintptr_t)bindings[b].resolved;
 return VirtualProtect(image+0xc89000-0x400000,4096,old,&ignored)!=0;
}
static int finite_words(const unsigned char *p,unsigned count){for(unsigned i=0;i<count;++i){float value;memcpy(&value,p+i*4,4);if(!isfinite(value))return 0;}return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"body_fault=%08lx ip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;if(!bind(image,0))return 5;
 DWORD ptd_slot=TlsAlloc(),cache_slot=TlsAlloc();if(ptd_slot==TLS_OUT_OF_INDEXES||cache_slot==TLS_OUT_OF_INDEXES)return 6;
 const unsigned pages[]={0xe50000,0xe56000,0xe58000,0xe85000};DWORD protection[4],ignored;unsigned char initial[4][4096];
 for(unsigned i=0;i<4;++i){memcpy(initial[i],image+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protection+i)||protection[i]!=PAGE_READONLY)return 7;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 8;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,total=0,magic=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 9;
 if(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31524249||fread(&total,4,1,input)!=1||!total||total>1024){error=10;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char linked[0x424],frame[64],a[16],b[16],center[16],body[0x1650]={0},wrapper[16]={0},ptd[0x214]={0};
  unsigned char globals[20];unsigned seed;
  if(fread(linked,1,sizeof(linked),input)!=sizeof(linked)||fread(frame,1,64,input)!=64||fread(a,1,16,input)!=16||fread(b,1,16,input)!=16||fread(center,1,16,input)!=16||
   fread(body+0x290,1,40,input)!=40||fread(body+0x2e0,1,4,input)!=4||fread(body+0x15a8,1,4,input)!=4||fread(globals,1,20,input)!=20||fread(&seed,4,1,input)!=1){error=11;break;}
  if(!finite_words(linked+0x210,8)||!finite_words(frame,16)||!finite_words(a,4)||!finite_words(b,4)||!finite_words(center,4)||!finite_words(body+0x290,10)||!finite_words(body+0x2e0,1)||
   !finite_words(body+0x15a8,1)||*(float *)(body+0x15a8)<=0||!finite_words(globals,5)||*(float *)(globals+8)<=0){error=12;break;}
  for(unsigned i=0;i<4;++i)memcpy(image+pages[i]-0x400000,initial[i],4096);
  memcpy(image+0xe500a4-0x400000,globals,20);*(DWORD *)(image+0xe56968-0x400000)=ptd_slot;*(DWORD *)(image+0xe5696c-0x400000)=cache_slot;
  *(unsigned *)(ptd+0x14)=seed;
  if(!TlsSetValue(ptd_slot,ptd)||!TlsSetValue(cache_slot,(void *)(uintptr_t)bindings[2].resolved)){error=13;break;}
  *(void **)(linked+0x334)=center;*(void **)(body+0xdb0)=wrapper;*(void **)(wrapper+8)=linked;
  *(void **)(image+0xe85f28-0x400000)=body;*(unsigned *)(image+0xe85fcc-0x400000)=1;
  Kernel kernel=(Kernel)(image+0x9d4210-0x400000);
  __try{kernel(linked,frame,a,b);}__except(fault(GetExceptionInformation())){error=14;}
  if(error)break;
  fwrite(linked+0x210,1,32,output);fwrite(a,1,16,output);fwrite(body+0x290,1,40,output);fwrite(body+0x2e0,1,4,output);fwrite(ptd+0x14,1,4,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=15;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=16;}
 for(unsigned i=0;i<4;++i){memcpy(image+pages[i]-0x400000,initial[i],4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,protection[i],&ignored))error=17;}
 if(!bind(image,1)||!TlsSetValue(ptd_slot,NULL)||!TlsSetValue(cache_slot,NULL)||!TlsFree(ptd_slot)||!TlsFree(cache_slot))error=18;
 if(ferror(input)||ferror(output))error=19;int ci=fclose(input),co=fclose(output);if(ci||co)error=19;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("impact_body_whole_cases=%u error=%u code_unchanged=%u real_imports=4 original_rand=1\n",cases,error,error!=16);return (int)error;
}
