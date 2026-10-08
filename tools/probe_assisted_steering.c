/* Whole original assisted9b3110/real feedback+math, supplied body/tuning. */
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
#ifdef GRID_ASSISTED_GEOMETRY
typedef void (__cdecl *GeometryKernel)(void *,void *);
#endif
#ifdef GRID_ASSISTED_WHEEL_SOURCE
typedef void (__cdecl *WheelKernel)(void *,void *,void *,unsigned);
#endif
#ifdef GRID_ASSISTED_CONTROL
typedef void (__fastcall *ControlKernel)(void *,void *);
#endif

int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 Kernel kernel=(Kernel)(image+0x9b3110-0x400000);
 DWORD saved_protection=0,ignored=0;
 if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&saved_protection))return 19;
 FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input||_wfopen_s(&output,argv[3],L"wx")||!output)return 5;
 char line[2048],name[64];unsigned cases=0,error=0,word_count=109;
#ifdef GRID_ASSISTED_GEOMETRY
 word_count=125;
#endif
#ifdef GRID_ASSISTED_WHEEL_SOURCE
 word_count=128;
#endif
#ifdef GRID_ASSISTED_CONTROL
 word_count=146;
#endif
 while(fgets(line,sizeof(line),input)){
  if(++cases>4096||!strchr(line,'\n')){error=6;break;}
  char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t n=(size_t)(cursor-start);
  if(!n||n>=sizeof(name)){error=7;break;}memcpy(name,start,n);name[n]=0;
  float v[146];for(unsigned i=0;i<word_count;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long word=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=8;break;}unsigned w=(unsigned)word;memcpy(v+i,&w,4);cursor=stop;if(!isfinite(v[i])){error=8;break;}}
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
#ifdef GRID_ASSISTED_WHEEL_SOURCE
  __declspec(align(16)) unsigned char wheel_properties[0x660]={0};
  memcpy(wheel_properties+0x424,v+125,4);memcpy(wheel_properties+0x428,v+126,4);memcpy(wheel_properties+0x42c,v+127,4);
  WheelKernel initialize_wheel=(WheelKernel)(image+0x9b6350-0x400000);
  for(unsigned i=0;i<4;++i){
   __try{initialize_wheel(body+0x1990+i*0x150,body+0x15a0,wheel_properties,i<2);}
   __except(EXCEPTION_EXECUTE_HANDLER){error=23;break;}
  }
  if(error)break;
#endif
  for(unsigned i=0;i<4;++i){float *q=v+13+i*10;unsigned char *w=body+0x1990+i*0x150;
   if((q[3]!=0.f&&q[3]!=1.f)||q[8]<0.f||q[8]>1.f){error=16;break;}
   memcpy(w,q,4);memcpy(body+0x22a0+i*4,q+2,4);body[0x22cc+i]=(unsigned char)q[3];
#ifndef GRID_ASSISTED_WHEEL_SOURCE
   memcpy(w+0xe8,q+1,4);memcpy(w+0xf0,q+8,4);
#endif
   memcpy(w+0x20,q+4,16);memcpy(w+0x144,q+9,4);
  }
  if(error)break;
  body[0xe6c]=1;memcpy(body+0x15e0,v+53,64);memcpy(body+0x15c0,v+69,16);memcpy(body+0x15d0,v+73,16);
  memcpy(body+0x1c30,v+77,16);memcpy(body+0x1d80,v+81,16);
  const unsigned parameters[20]={0xfdc,0x138c,0x123c,0x1240,0x1244,0xe70,0xe5c,0xe58,0x101c,0x1020,0x1024,0x1028,0xfec,0xfe8,0x1248,0x1250,0x124c,0x1254,0x1388,0x1f2c};
  for(unsigned i=0;i<20;++i)memcpy(body+parameters[i],v+85+i,4);
  for(unsigned i=0;i<4;++i)memcpy(body+0x1990+i*0x150+0x38,v+105+i,4);
#ifdef GRID_ASSISTED_GEOMETRY
  __declspec(align(16)) unsigned char properties[0x400]={0};unsigned pair[2]={0};
  memcpy(properties+0x340,v+4,4);
  for(unsigned i=0;i<4;++i)memcpy(body+0x1990+i*0x150,v+109+i*4,16);
  GeometryKernel geometry=(GeometryKernel)(image+0x98ebf0-0x400000);
  __try{geometry(body,properties);}__except(EXCEPTION_EXECUTE_HANDLER){error=21;break;}
  for(unsigned i=0;i<2;++i){uintptr_t p=*(unsigned *)(body+0x21cc+i*4),first=(uintptr_t)(body+0x1990);
   if(p<first||p>first+3*0x150||(p-first)%0x150){error=22;break;}pair[i]=(unsigned)((p-first)/0x150);
  }
  if(error)break;
#endif
#ifdef GRID_ASSISTED_CONTROL
  __declspec(align(16)) unsigned char control_state[0x460]={0},control_wrapper[0x30]={0},control_body[0x1640]={0};
  float *q=v+128;
  if((q[17]!=0.f&&q[17]!=1.f)||q[6]<0.f||q[6]>1.f||q[7]<0.f||q[7]>1.f){error=24;break;}
  memcpy(control_state+0x18,q,16);memcpy(control_state+0x30,q+4,8);memcpy(control_state+0x48,q+6,8);memcpy(control_state+0x50,q+8,16);
  memcpy(control_state+0x270,q+12,16);memcpy(control_state+0x254,q+16,4);
  if(q[17]!=0.f){*(unsigned *)(control_state+0x214)=(unsigned)(uintptr_t)control_wrapper;*(unsigned *)(control_wrapper+0x24)=(unsigned)(uintptr_t)control_body;*(unsigned *)(control_body+0x1620)=4;}
  ControlKernel control_kernel=(ControlKernel)(image+0x9c5b90-0x400000);
  __try{control_kernel(control_state,NULL);}__except(EXCEPTION_EXECUTE_HANDLER){error=25;break;}
  /* Steering-only transfer with aligned static provenance; whole 9bd7f0 is unexecuted. */
  memcpy(body+0x2294,control_state+0x3ac,12);
  for(unsigned i=0;i<4;++i){memcpy(body+0x22a0+i*4,control_state+0x3d4+i*24,4);body[0x22cc+i]=control_state[0x3dc+i*24];}
#endif
  __try{kernel(body,v[10]);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
  fprintf(output,"P\t%s",name);
#ifdef GRID_ASSISTED_CONTROL
  const unsigned control_offsets[7]={0x3ac,0x3b0,0x3b4,0x3d4,0x3ec,0x404,0x41c};
  for(unsigned i=0;i<7;++i){float value=*(float *)(control_state+control_offsets[i]);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  for(unsigned i=0;i<4;++i)fprintf(output,"\t%u",control_state[0x3dc+i*24]);
#endif
#ifdef GRID_ASSISTED_GEOMETRY
  fprintf(output,"\t%08x\t%u\t%u\t%08x\t%08x",bits(*(float *)(body+0xe54)),pair[0],pair[1],bits(*(float *)(body+0x21d4)),bits(*(float *)(body+0x21d8)));
#endif
#ifdef GRID_ASSISTED_WHEEL_SOURCE
  for(unsigned i=0;i<4;++i){unsigned char *w=body+0x1990+i*0x150;fprintf(output,"\t%08x\t%08x",bits(*(float *)(w+0xe8)),bits(*(float *)(w+0xf0)));}
#endif
  for(unsigned i=0;i<4;++i){unsigned char *w=body+0x1990+i*0x150;
   unsigned offsets[5]={0x20,0x24,0x28,0x2c,0x38};
   for(unsigned j=0;j<5;++j){float value=*(float *)(w+offsets[j]);if(!isfinite(value))error=11;fprintf(output,"\t%08x",bits(value));}
  }fputc('\n',output);if(error)break;
 }
 if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;
 if(!VirtualProtect(image+0xe58000-0x400000,0x1000,saved_protection,&ignored))error=20;
 VirtualFree(image,0,MEM_RELEASE);printf("assisted_steering_cases=%u error=%u\n",cases,error);return error;
}
