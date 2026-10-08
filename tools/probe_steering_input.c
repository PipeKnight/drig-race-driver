/* Whole original 98f790, real trigonometry, supplied body scalars. */
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
static void invoke(void *kernel,void *body,float *command){
 __asm { push esi }
 __asm { mov eax,body }
 __asm { mov esi,command }
 __asm { call kernel }
 __asm { pop esi }
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 void *kernel=image+0x98f790-0x400000;
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[256],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[4];for(unsigned i=0;i<4;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  __declspec(align(16)) unsigned char body[0x2300]={0};
  memcpy(body+0x2294,v+1,4);memcpy(body+0x1648,v+2,4);memcpy(body+0x159c,v+3,4);
  __try{invoke(kernel,body,v);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  if(!isfinite(v[0])){error=11;break;}
  fprintf(output,"P\t%s\t%08x\n",name,bits(v[0]));
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("steering_input_cases=%u error=%u\n",cases,error);return error;
}
