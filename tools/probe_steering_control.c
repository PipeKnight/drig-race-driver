/* Whole mutable-state producer, scoped steering outputs and inactive component queries. */
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
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9c5b90-0x400000);
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[512],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[18];for(unsigned i=0;i<18;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  if(v[17]!=0.f&&v[17]!=1.f){error=13;break;}
  if(v[6]<0.f||v[6]>1.f||v[7]<0.f||v[7]>1.f){error=14;break;}
  __declspec(align(16)) unsigned char state[0x460]={0},wrapper[0x30]={0},body[0x1640]={0};
  memcpy(state+0x18,v,16);memcpy(state+0x30,v+4,8);memcpy(state+0x48,v+6,8);memcpy(state+0x50,v+8,16);
  memcpy(state+0x270,v+12,16);memcpy(state+0x254,v+16,4);
  if(v[17]!=0.f){*(unsigned *)(state+0x214)=(unsigned)(uintptr_t)wrapper;*(unsigned *)(wrapper+0x24)=(unsigned)(uintptr_t)body;*(unsigned *)(body+0x1620)=4;}
  __try{kernel(state,NULL);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  fprintf(output,"P\t%s",name);const unsigned offsets[7]={0x3ac,0x3b0,0x3b4,0x3d4,0x3ec,0x404,0x41c};
  for(unsigned i=0;i<7;++i){float value=*(float *)(state+offsets[i]);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  for(unsigned i=0;i<4;++i)fprintf(output,"\t%u",state[0x3dc+i*24]);
  fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("steering_control_cases=%u error=%u\n",cases,error);return error;
}
