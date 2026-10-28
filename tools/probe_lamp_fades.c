/* Execute unchanged source lamp update on owned state, with no draw/material graph. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <float.h>
#include <math.h>
#include "isolated_reference.h"
static void word(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *in=NULL;if(_wfopen_s(&in,argv[2],L"rb")||!in)return 4;
 unsigned header[2];if(fread(header,4,2,in)!=2||header[0]!=0x314c5047||!header[1]||header[1]>4096){fclose(in);return 5;}
 unsigned count=header[1];float (*cases)[10]=calloc(count,sizeof(*cases));unsigned (*result)[4]=calloc(count,sizeof(*result));
 if(!cases||!result){fclose(in);free(cases);free(result);return 6;}
 int valid=fread(cases,sizeof(*cases),count,in)==count&&fgetc(in)==EOF;fclose(in);if(!valid){free(cases);free(result);return 7;}
 for(unsigned i=0;i<count;i++)for(unsigned j=0;j<10;j++)if(!isfinite(cases[i][j])||fabsf(cases[i][j])>9.f){free(cases);free(result);return 8;}
 unsigned char *image=map_reference(argv[1]);if(!image){free(cases);free(result);return 9;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned size=pe->OptionalHeader.SizeOfImage;
 unsigned char *snapshot=malloc(size);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);free(cases);free(result);return 10;}memcpy(snapshot,image,size);
 __declspec(align(16)) unsigned char self[0x3a00],vehicle[0x2600],groups[32];unsigned error=0;
 unsigned char before[0x3a00],vehicle_before[0x2600];
 for(unsigned i=0;i<count&&!error;i++){
  const float *c=cases[i];if(fabsf(c[0])>1.f||(c[7]!=0.f&&c[7]!=1.f)||c[8]<0.f||c[8]>1.f||(c[9]!=1.f&&c[9]!=9.f)){error=11;break;}
  memset(self,0,sizeof(self));memset(vehicle,0,sizeof(vehicle));memset(groups,0,sizeof(groups));
  for(unsigned j=0;j<3;j++){if(c[1+j]<0.f||c[1+j]>2.f||c[4+j]<0.f||c[4+j]>2.f)error=12;word(self,0x32a0+j*4,bits(c[1+j]));}
  if(error)break;self[0x32b0]=(unsigned char)c[7];word(vehicle,0xe20,bits(c[8]));word(vehicle,0x196c,(unsigned)c[9]);
  memcpy(before,self,sizeof(self));memcpy(vehicle_before,vehicle,sizeof(vehicle));
  __try{((void (__fastcall *)(void *,void *,float,void *,void *,float,float,float))(image+0x9024c0-0x400000))(self,NULL,c[0],vehicle,groups,c[4],c[5],c[6]);}
  __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"lamp_fault=%08lx case=%u\n",GetExceptionCode(),i);error=13;}
  if(memcmp(before,self,0x32a0)||memcmp(before+0x32b0,self+0x32b0,sizeof(self)-0x32b0)||memcmp(vehicle_before,vehicle,sizeof(vehicle)))error=17;
  for(unsigned j=0;j<sizeof(groups);j++)if(groups[j])error=18;
  memcpy(result[i],self+0x32a0,16);
 }
 unsigned unchanged=memcmp(snapshot,image,size)==0;if(!unchanged)error=14;
 if(!error){HANDLE out=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(out==INVALID_HANDLE_VALUE)error=15;else{DWORD written=0;DWORD bytes=count*(DWORD)sizeof(*result);if(!WriteFile(out,result,bytes,&written,NULL)||written!=bytes)error=16;CloseHandle(out);}}
 printf("lamp_cases=%u image_unchanged=%u code_replacements=0 draw_graph=absent error=%u\n",count,unchanged,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);free(cases);free(result);return (int)error;
}
