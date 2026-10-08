/* Whole original 9b2790 and original cross helper, supplied four-wheel state. */
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
typedef void (__cdecl *Kernel)(void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9b2790-0x400000);
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[2048],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[112];for(unsigned i=0;i<112;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  __declspec(align(16)) unsigned char body[0x2300]={0},view[0x300]={0},matrices[4][0x80]={0};
  *(unsigned *)body=(unsigned)(uintptr_t)view;*(unsigned *)(body+0x1620)=4;
  memcpy(body+0x15e0,v,16);memcpy(body+0x1600,v+4,16);memcpy(body+0x15c0,v+8,32);
  for(unsigned i=0;i<4;++i){float *q=v+16+i*24;unsigned char *w=body+0x1990+i*0x150;
   memcpy(w,q,16);*(float *)(w+0x40)=q[4];*(float *)(w+0xe0)=q[5];*(float *)(w+0x20)=q[6];*(float *)(w+0x24)=q[7];
   *(unsigned *)(w+0x34)=(unsigned)(uintptr_t)matrices[i];memcpy(matrices[i]+0x40,q+8,64);
  }
  __try{kernel(body);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  fprintf(output,"P\t%s",name);for(unsigned i=0;i<4;++i)for(unsigned j=0;j<4;++j)for(unsigned k=0;k<4;++k){float f=*(float *)(view+0x190+j*0x40+i*0x10+k*4);if(!isfinite(f))error=11;fprintf(output,"\t%08x",bits(f));}fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("force_view_cases=%u error=%u\n",cases,error);return error;
}
