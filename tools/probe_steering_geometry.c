/* Whole original 98ebf0, four supplied wheel coordinates and property340. */
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
typedef void (__cdecl *Kernel)(void *,void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x98ebf0-0x400000);
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[512],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[17];for(unsigned i=0;i<17;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  __declspec(align(16)) unsigned char body[0x2300]={0},properties[0x400]={0};
  *(unsigned *)(body+0x1620)=4;memcpy(properties+0x340,v,4);
  for(unsigned i=0;i<4;++i)memcpy(body+0x1990+i*0x150,v+1+i*4,16);
  __try{kernel(body,properties);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  unsigned pairs[2];for(unsigned i=0;i<2;++i){uintptr_t p=*(unsigned *)(body+0x21cc+i*4),start_body=(uintptr_t)(body+0x1990);
   if(p<start_body||p>start_body+3*0x150||(p-start_body)%0x150){error=11;break;}pairs[i]=(unsigned)((p-start_body)/0x150);
  }
  if(error)break;
  float width=*(float *)(body+0x21d4),separation=*(float *)(body+0x21d8);if(!isfinite(width)||!isfinite(separation)){error=11;break;}
  fprintf(output,"P\t%s\t%08x\t%u\t%u\t%08x\t%08x\n",name,bits(*(float *)(body+0xe54)),pairs[0],pairs[1],bits(width),bits(separation));
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("steering_geometry_cases=%u error=%u\n",cases,error);return error;
}
