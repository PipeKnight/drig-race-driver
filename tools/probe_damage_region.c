/* Whole original localized-damage helper on bounded owned records.
   No original function replacements, game process or borrowed component graph. */
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
static unsigned word(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static float number(const void *p,unsigned o){float v;memcpy(&v,(const char *)p+o,4);return v;}
static int bound(const void *p,unsigned o,float low,float high){float v=number(p,o);return isfinite(v)&&v>=low&&v<=high;}
static int canary(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+n+i]!=0xcd)return 0;return 1;}
static uintptr_t reference;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"damage_region_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[2];if(fread(h,4,2,file)!=2||h[0]!=0x31304d44||!h[1]||h[1]>16384){fclose(file);return 5;}
 unsigned count=h[1],size=count*128;unsigned char *inputs=malloc(size),*outputs=malloc(count*8);
 if(!inputs||!outputs){fclose(file);free(inputs);free(outputs);return 6;}
 if(fread(inputs,1,size,file)!=size||fgetc(file)!=EOF||ferror(file)){fclose(file);free(inputs);free(outputs);return 7;}fclose(file);
 for(unsigned i=0;i<count;i++){
  const unsigned char *r=inputs+i*128;
  if(!bound(r,0,0.f,1.5f)||!bound(r,4,0.f,1.f)||!bound(r,8,0.f,1000.f)||word(r,12)>1||!bound(r,64,0.f,1000.f)||!bound(r,68,0.f,1.f)||!bound(r,72,0.f,1.f)||!bound(r,96,0.f,1000.f)||!bound(r,100,0.f,1000.f)||!bound(r,104,0.f,1000.f)){free(inputs);free(outputs);return 8;}
  for(unsigned j=0;j<4;j++)if(!bound(r,16+j*4,-10000.f,10000.f)||!bound(r,32+j*4,-10000.f,10000.f)||!bound(r,48+j*4,0.f,100.f)){free(inputs);free(outputs);return 8;}
  if(number(r,28)!=1.f||(number(r,44)!=0.f&&number(r,44)!=1.f)||(number(r,60)!=0.f&&number(r,60)!=1.f)){free(inputs);free(outputs);return 8;}
  const unsigned reserved[]={76,80,84,88,92,108,116,120,124};for(unsigned j=0;j<sizeof(reserved)/sizeof(reserved[0]);j++)if(word(r,reserved[j])){free(inputs);free(outputs);return 8;}
 }
 unsigned char *image=map_reference(argv[1]);if(!image){free(inputs);free(outputs);return 9;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *snapshot=malloc(image_size);if(!snapshot)return 10;memcpy(snapshot,image,image_size);
 const unsigned pages[]={0xe58000,0xe4f000};DWORD prior[2],ignored;unsigned protected=0,error=0;
 for(unsigned i=0;i<2;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,prior+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  __declspec(align(16)) unsigned char record_owned[32+128+32],box_owned[32+0x30+32];
  unsigned char *r=record_owned+32,*box=box_owned+32;memset(record_owned,0xcd,sizeof(record_owned));memset(box_owned,0xcd,sizeof(box_owned));memcpy(r,inputs+i*128,128);memset(box,0,0x30);memcpy(box,r+32,32);box[0x20]=(unsigned char)word(r,12);
  memcpy(image+0xe58000-0x400000,snapshot+0xe58000-0x400000,4096);
  memcpy(image+0xe4fcc0-0x400000,r+112,4);
  __try{((void (__cdecl *)(void *,void *,float,void *,float,void *,void *))(image+0x999c00-0x400000))(r,r+16,number(r,4),r+64,number(r,8),box,r+80);}
  __except(fault(GetExceptionInformation())){error=12;}
  if(memcmp(r+4,inputs+i*128+4,124)||memcmp(box,inputs+i*128+32,32)||box[0x20]!=word(r,12)||!canary(record_owned,128)||!canary(box_owned,0x30))error=13;
  for(unsigned j=0x21;j<0x30;j++)if(box[j])error=13;
  if(!isfinite(number(r,0)))error=14;
  memcpy(outputs+i*8,r,4);memcpy(outputs+i*8+4,image+0xe4fcc0-0x400000,4);
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,prior[i],&ignored))error=15;}
 if(memcmp(image,snapshot,image_size))error=16;
 if(!error){HANDLE out=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(out==INVALID_HANDLE_VALUE)error=17;else{DWORD written;if(!WriteFile(out,outputs,count*8,&written,NULL)||written!=count*8)error=18;if(!CloseHandle(out))error=19;}}
 printf("damage_regions=%u code_unchanged=%u globals_restored=%u code_replacements=0 error=%u\n",count,error!=16,error!=15&&error!=16,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return error;
}
