/* Whole original 9b3110, assistance disabled; real input/trig helpers. */
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
typedef void (__cdecl *Kernel)(void *,float);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9b3110-0x400000);
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[1024],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[53];for(unsigned i=0;i<53;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  if(v[9]!=0.f&&v[9]!=1.f){error=13;break;}
  if(v[10]<0.f||v[4]<0.f||v[5]<0.f||v[6]<=0.f||v[7]<0.f||v[8]<0.f){error=14;break;}
  for(unsigned i=11;i<13;++i)if(v[i]<0.f||v[i]>3.f||v[i]!=(float)(unsigned)v[i])error=15;
  if(error||v[11]==v[12]){error=15;break;}
  __declspec(align(16)) unsigned char body[0x2300]={0};
  memcpy(body+0xe30,v,4);memcpy(body+0x2294,v+1,4);memcpy(body+0x1648,v+2,4);memcpy(body+0x159c,v+3,4);
  memcpy(body+0xe54,v+4,4);memcpy(body+0x21d4,v+5,8);memcpy(body+0x2298,v+7,8);*(unsigned *)(body+0x1f74)=(unsigned)v[9];
  *(unsigned *)(body+0x1620)=4;
  *(unsigned *)(body+0x21cc)=(unsigned)(uintptr_t)(body+0x1990+(unsigned)v[11]*0x150);
  *(unsigned *)(body+0x21d0)=(unsigned)(uintptr_t)(body+0x1990+(unsigned)v[12]*0x150);
  for(unsigned i=0;i<4;++i){float *q=v+13+i*10;unsigned char *w=body+0x1990+i*0x150;
   if((q[3]!=0.f&&q[3]!=1.f)||q[8]<0.f||q[8]>1.f){error=16;break;}
   memcpy(w,q,4);memcpy(w+0xe8,q+1,4);memcpy(body+0x22a0+i*4,q+2,4);body[0x22cc+i]=(unsigned char)q[3];
   memcpy(w+0x20,q+4,16);memcpy(w+0xf0,q+8,4);memcpy(w+0x144,q+9,4);
  }
  if(error)break;
  __try{kernel(body,v[10]);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  fprintf(output,"P\t%s",name);for(unsigned i=0;i<4;++i){unsigned char *w=body+0x1990+i*0x150;
   unsigned offsets[5]={0x20,0x24,0x28,0x2c,0x38};
   for(unsigned j=0;j<5;++j){float value=*(float *)(w+offsets[j]);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  }fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("steering_coefficient_cases=%u error=%u\n",cases,error);return error;
}
