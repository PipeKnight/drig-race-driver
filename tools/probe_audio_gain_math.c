/* Verify private native math contracts used by the original gain/crossfade path.
   This executes unchanged CRT helpers, not whole 42DE60 or an audio backend. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <float.h>
#include <math.h>
#include <io.h>
#include <fcntl.h>
#include "isolated_reference.h"
static void invoke(void *code,const double *input,double *output){
 __asm {
  mov eax,code
  mov ecx,input
  movsd xmm0,qword ptr [ecx]
  call eax
  mov ecx,output
  movsd qword ptr [ecx],xmm0
 }
}
static int private_file(const wchar_t *path){
 wchar_t root[32768],full[32768];DWORD a=GetFullPathNameW(L"local\\",32768,root,NULL),b=GetFullPathNameW(path,32768,full,NULL);
 return a&&a<32768&&b&&b<32768&&!_wcsnicmp(root,full,wcslen(root));
}
static unsigned fault(EXCEPTION_POINTERS *p){fprintf(stderr,"exception=%08lx instruction=%p\n",p->ExceptionRecord->ExceptionCode,p->ExceptionRecord->ExceptionAddress);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
 if(argc!=3||!private_file(argv[1])||!private_file(argv[2]))return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;
 memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
 HANDLE file=CreateFileW(argv[2],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
 if(file==INVALID_HANDLE_VALUE){free(saved);VirtualFree(image,0,MEM_RELEASE);return 5;}
 int descriptor=_open_osfhandle((intptr_t)file,_O_BINARY);if(descriptor<0){CloseHandle(file);return 5;}
 FILE *output=_fdopen(descriptor,"wb");if(!output){_close(descriptor);return 5;}
 unsigned header[]={0x31304d47,4097};unsigned error=0,completed=0;
 if(fwrite(header,sizeof(header),1,output)!=1)error=6;
 for(unsigned i=0;!error&&i<header[1];i++){
  double values[6];float t=.001f+(.998f*(float)i)/4096.f;
  float angle=t* *(float *)(image+0xcb22c0-0x400000);double input=(double)angle;
  values[0]=(double)t;
  __try {
   invoke(image+0xbf4e44-0x400000,&input,values+1);
   invoke(image+0xbf523a-0x400000,&input,values+2);
   invoke(image+0xbf4ffc-0x400000,values,values+3);
   /* Observed contracts: BF4E44=sin, BF523A=cos, BF4FFC=log10. */
   double sine=(double)(float)values[1],cosine=(double)(float)values[2];
   if(cosine<=0||sine<=0){error=7;break;}
   invoke(image+0xbf4ffc-0x400000,&sine,values+4);
   invoke(image+0xbf4ffc-0x400000,&cosine,values+5);
  }__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  for(unsigned j=0;j<6;j++)if(!isfinite(values[j]))error=8;
  if(!error&&fwrite(values,sizeof(values),1,output)!=1)error=9;
  float gains[2]={(float)values[5]*20.f,(float)values[4]*20.f};
  for(unsigned j=0;j<2;j++)if(gains[j]<-48.f)gains[j]=-48.f;
  if(!error&&fwrite(gains,sizeof(gains),1,output)!=1)error=9;
  if(!error)completed++;
 }
 if(fclose(output))error=10;
 unsigned unchanged=memcmp(image,saved,pe->OptionalHeader.SizeOfImage)==0;if(!unchanged)error=11;
 printf("audio_gain_math_cases=%u completed=%u entire_image_unchanged=%u error=%u\n",header[1],completed,unchanged,error);
 free(saved);VirtualFree(image,0,MEM_RELEASE);return (int)error;
}
