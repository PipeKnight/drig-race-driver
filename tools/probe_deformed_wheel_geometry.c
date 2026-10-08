/* Whole original9bb1e0 with real sampler/math and supplied four-wheel body. */
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
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 reference_base=(uintptr_t)image;
 Kernel kernel=(Kernel)(image+0x9bb1e0-0x400000);
 DWORD protections[4],ignored=0;unsigned pages[4]={0xe58000,0xe5a000,0xe5b000,0xe5e000};
 for(unsigned i=0;i<4;++i)if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protections+i))return 19;
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[16384],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>1024||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[1773];unsigned words=0;
  while(*cursor&&*cursor!='\r'&&*cursor!='\n'){
   while(*cursor=='\t'||*cursor==' ')++cursor;
   if(words>=1773){error=8;break;}char *stop;unsigned long w=strtoul(cursor,&stop,16);
   if(stop-cursor!=8){error=8;break;}unsigned word=(unsigned)w;memcpy(v+words,&word,4);
   if(!isfinite(v[words])){error=8;break;}++words;cursor=stop;
  }
  if(error||words<173){error=9;break;}
  unsigned dimensions[3],count=1;
  for(unsigned i=0;i<3;++i){if(v[i]<2.f||v[i]>16.f||v[i]!=(float)(unsigned)v[i]){error=13;break;}dimensions[i]=(unsigned)v[i];count*=dimensions[i];}
  if(error||count>400||words!=173+count*4||v[3]<=0.f||(v[16]!=0.f&&v[16]!=1.f)){error=14;break;}
  __declspec(align(16)) unsigned char body[0xf000]={0};
  *(unsigned *)(body+0x1620)=4;
  memcpy(body+0xc714,dimensions,12);memcpy(body+0xc720,v+3,4);memcpy(body+0xc730,v+4,16);
  memcpy(body+0x1630,v+8,16);memcpy(body+0xeefc,v+12,4);memcpy(body+0xd070,v+13,4);memcpy(body+0xe25c,v+14,8);body[0xc7ad]=(unsigned char)v[16];
  for(unsigned i=0;i<4;++i){float *q=v+17+i*8;unsigned char *w=body+0x1990+i*0x150;
   if(q[4]<=0.f||q[5]<=0.f||q[6]<=0.f||(q[7]!=0.f&&q[7]!=1.f)){error=15;break;}
   memcpy(w,q,16);memcpy(w+0xe0,q+4,4);memcpy(w+0xf4,q+5,8);body[0x22e0+i]=(unsigned char)q[7];
  }if(error)break;
  memcpy(body+0x1650,v+49,64);memcpy(body+0x1790,v+65,16);memcpy(body+0x1690,v+69,256);memcpy(body+0x30c,v+133,160);
  for(unsigned i=0;i<count;++i)memcpy(body+0x4880+i*0x50,v+173+i*4,16);
  __try{kernel(body,NULL);}__except(fault_details(GetExceptionInformation())){error=10;break;}
  fprintf(output,"P\t%s",name);
  unsigned offsets[4]={0x1650,0x1790,0x1690,0x30c},lengths[4]={16,4,64,40};
  for(unsigned j=0;j<4;++j)for(unsigned i=0;i<lengths[j];++i){float value=*(float *)(body+offsets[j]+i*4);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  for(unsigned i=0;i<4;++i)fprintf(output,"\t%u",body[0x22e0+i]);fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 for(unsigned i=0;i<4;++i)if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=20;
 VirtualFree(image,0,MEM_RELEASE);printf("deformed_wheel_geometry_cases=%u error=%u\n",cases,error);return error;
}
