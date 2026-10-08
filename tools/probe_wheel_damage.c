/* Whole original one-wheel visual update; component notifications inactive. */
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
/* ECX settings, unused EDX, three stack args; original callee pops 12 bytes. */
typedef void (__fastcall *Kernel)(void *,void *,float,unsigned,void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9f2f80-0x400000);
 DWORD saved=0,ignored=0;
 if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&saved))return 19;
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[512],name[64];unsigned cases=0,error=0;
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  unsigned word_count=19;
#ifdef GRID_WHEEL_DAMAGE_SOURCE
  word_count=20;
#endif
  float v[20];for(unsigned i=0;i<word_count;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
  while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=9;break;}
  if(v[4]<0.f||v[4]>3.f||v[4]!=(float)(unsigned)v[4]||(v[15]!=0.f&&v[15]!=1.f)){error=13;break;}
  if(v[16]<0.f||(v[16]!=0.f&&v[17]<=0.f)){error=14;break;}
  __declspec(align(16)) unsigned char state[0x460]={0},wrapper[0x30]={0},body[0xef00]={0};
  unsigned wheel=(unsigned)v[4];
  memcpy(state+0x18,v,16);memcpy(state+0x280,v+16,8);
#ifdef GRID_WHEEL_DAMAGE_SOURCE
  memcpy(state+0x288,v+19,4);
#endif
  *(unsigned *)(state+0x214)=(unsigned)(uintptr_t)wrapper;*(unsigned *)(wrapper+0x24)=(unsigned)(uintptr_t)body;
  memcpy(body+0x1790+wheel*4,v+5,4);memcpy(body+0x1650+wheel*16,v+6,16);
  memcpy(body+0xc740,v+10,16);memcpy(body+0xeefc,v+14,4);body[0xc7ae]=(unsigned char)v[15];
  /* Status7 tests the exact 442=false guard without calling game components. */
  *(unsigned *)(state+0x444)=7;
  __try{kernel(state+0x258,NULL,v[18],wheel,state);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  if(*(unsigned *)(state+0x444)!=7||state[0x448]!=0){error=15;break;}
  fprintf(output,"P\t%s",name);
#ifdef GRID_WHEEL_DAMAGE_SOURCE
  for(unsigned i=0;i<3;++i)fprintf(output,"\t%08x",bits(*(float *)(state+0x280+i*4)));
#endif
  for(unsigned i=0;i<4;++i){float value=*(float *)(state+0x18+i*4);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 if(!VirtualProtect(image+0xe58000-0x400000,0x1000,saved,&ignored))error=20;
 VirtualFree(image,0,MEM_RELEASE);printf("wheel_damage_cases=%u error=%u\n",cases,error);return error;
}
