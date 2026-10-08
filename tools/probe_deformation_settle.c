/* Whole original9a26f0/9a2110 on a supplied bounded400-node pool. */
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
typedef void (__fastcall *Kernel)(void *,void *);
static uintptr_t reference_base;
static int fault_details(EXCEPTION_POINTERS *p){
 fprintf(stderr,"fault=%08lx at_reference=%08lx access=%lu target_reference=%08lx\n",p->ExceptionRecord->ExceptionCode,
  (unsigned long)((uintptr_t)p->ExceptionRecord->ExceptionAddress-reference_base+0x400000),
  (unsigned long)p->ExceptionRecord->ExceptionInformation[0],
  (unsigned long)(p->ExceptionRecord->ExceptionInformation[1]-reference_base+0x400000));
 return EXCEPTION_EXECUTE_HANDLER;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;reference_base=(uintptr_t)image;
 Kernel kernel=(Kernel)(image+0x9a26f0-0x400000);
 DWORD protection,ignored;if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&protection))return 19;
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[80000],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>1024||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[8408];unsigned words=0;
  while(*cursor&&*cursor!='\r'&&*cursor!='\n'){
   while(*cursor=='\t'||*cursor==' ')++cursor;
   if(words>=8408){error=8;break;}char *stop;unsigned long w=strtoul(cursor,&stop,16);
   if(stop-cursor!=8){error=8;break;}unsigned word=(unsigned)w;memcpy(v+words,&word,4);
   if(!isfinite(v[words])){error=8;break;}++words;cursor=stop;
  }
  if(error||words<4808){error=9;break;}
  unsigned dimensions[3],count=1;
  for(unsigned i=0;i<3;++i){if(v[i]<2.f||v[i]>16.f||v[i]!=(float)(unsigned)v[i]){error=13;break;}dimensions[i]=(unsigned)v[i];count*=dimensions[i];}
  if(error||count>400||v[3]<=0.f||v[4]<0.f||v[4]>2.f||v[4]!=(float)(unsigned)v[4]||v[7]<0.f||v[7]>1200.f||v[7]!=(float)(unsigned)v[7]){error=14;break;}
  unsigned edges=(unsigned)v[7];if(words!=4808+edges*3){error=14;break;}
  __declspec(align(16)) unsigned char descriptor[0xa500]={0},settings[0x1030]={0};
  memcpy(descriptor+0xa414,dimensions,12);memcpy(descriptor+0xa420,v+3,4);
  *(unsigned *)(descriptor+0xa4a0)=(unsigned)v[4];*(unsigned *)(descriptor+0xa410)=edges;
  *(void **)(descriptor+0xa4f8)=settings;memcpy(settings+0x1024,v+5,8);
  for(unsigned i=0;i<400;++i){
   float *q=v+8+i*12;
   for(unsigned j=0;j<4;++j)if(q[8+j]<0.f){error=15;break;}
   memcpy(descriptor+0x2580+i*80,q,48);
  }if(error)break;
  for(unsigned i=0;i<edges;++i){float *q=v+4808+i*3;
   if(q[0]<0.f||q[0]>=400.f||q[0]!=(float)(unsigned)q[0]||q[1]<0.f||q[1]>=400.f||q[1]!=(float)(unsigned)q[1]){error=16;break;}
   *(short *)(descriptor+i*8)=(short)q[0];*(short *)(descriptor+i*8+2)=(short)q[1];memcpy(descriptor+i*8+4,q+2,4);
  }if(error)break;
  __try{kernel(descriptor,NULL);}__except(fault_details(GetExceptionInformation())){error=10;break;}
  fprintf(output,"P\t%s",name);
  for(unsigned i=0;i<400;++i)for(unsigned j=0;j<4;++j){float value=*(float *)(descriptor+0x2580+i*80+j*4);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  for(unsigned i=0;i<edges;++i){float value=*(float *)(descriptor+i*8+4);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 if(!VirtualProtect(image+0xe58000-0x400000,0x1000,protection,&ignored))error=20;
 VirtualFree(image,0,MEM_RELEASE);printf("deformation_settle_cases=%u error=%u\n",cases,error);return error;
}
