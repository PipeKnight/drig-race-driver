/* Whole original9a39d0 with supplied descriptor/node positions, no stubs. */
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
typedef void (__fastcall *Kernel)(void *,void *,void *,void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9a39d0-0x400000);
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[16384],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>1024||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[1612];unsigned words=0;
  while(*cursor&&*cursor!='\r'&&*cursor!='\n'){
   while(*cursor=='\t'||*cursor==' ')++cursor;
   if(words>=1612){error=8;break;}char *stop;unsigned long w=strtoul(cursor,&stop,16);
   if(stop-cursor!=8){error=8;break;}unsigned word=(unsigned)w;memcpy(v+words,&word,4);
   if(!isfinite(v[words])){error=8;break;}++words;cursor=stop;
  }
  if(error||words<12){error=9;break;}
  unsigned dimensions[3],count=1;
  for(unsigned i=0;i<3;++i){if(v[i]<2.f||v[i]>16.f||v[i]!=(float)(unsigned)v[i]){error=13;break;}dimensions[i]=(unsigned)v[i];count*=dimensions[i];}
  if(error||count>400||words!=12+count*4||v[3]<=0.f){error=14;break;}
  float inverse=1.f/v[3];
  for(unsigned i=0;i<3;++i){float lower=v[4+i],upper=((float)(dimensions[i]-1)-0.0010000000474974513f)*v[3]+lower;
   float clamped=v[8+i];if(clamped<lower)clamped=lower;if(clamped>upper)clamped=upper;
   float scaled=inverse*(clamped-lower);
   if(!isfinite(upper)||upper<=lower||!isfinite(scaled)||scaled<0.f||scaled>=(float)(dimensions[i]-1)){error=15;break;}
  }if(error)break;
  __declspec(align(16)) unsigned char descriptor[0xa440]={0};
  __declspec(align(16)) float point[4],result[4]={0};
  memcpy(descriptor+0xa414,dimensions,12);memcpy(descriptor+0xa420,v+3,4);memcpy(descriptor+0xa430,v+4,16);memcpy(point,v+8,16);
  for(unsigned i=0;i<count;++i)memcpy(descriptor+0x2580+i*0x50,v+12+i*4,16);
  __try{kernel(descriptor,NULL,result,point);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  fprintf(output,"P\t%s",name);for(unsigned i=0;i<4;++i){if(!isfinite(result[i]))error=11;fprintf(output,"\t%08x",bits(result[i]));}fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 VirtualFree(image,0,MEM_RELEASE);printf("deformation_sample_cases=%u error=%u\n",cases,error);return error;
}
