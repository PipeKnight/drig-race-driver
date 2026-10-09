/* Whole original9A14D0, supplied ordered contacts, no stubs/imports/code writes. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef void (__cdecl *Reduce)(void *,void **,int,void *,void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>4*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31524349||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  unsigned count;__declspec(align(16)) unsigned char contacts[80][0x80]={0};__declspec(align(16)) float point[4],direction[4];void *pointers[80];
  if(fread(&count,4,1,input)!=1||!count||count>80){error=7;break;}
  for(unsigned i=0;i<count;++i){unsigned reverse;float values[9];if(fread(&reverse,4,1,input)!=1||reverse>1||fread(values,4,9,input)!=9){error=8;break;}
   for(unsigned j=0;j<9;++j)if(!isfinite(values[j]))error=8;if(error)break;
   *(unsigned *)(contacts[i]+8)=reverse?0x10000:0;memcpy(contacts[i]+0x3c,values,4);memcpy(contacts[i]+0x50,values+1,32);pointers[i]=contacts[i];
  }
  if(error)break;Reduce reduce=(Reduce)(image+0x9a14d0-0x400000);
  __try{reduce((void *)(uintptr_t)0x10000,pointers,(int)count,direction,point);}__except(EXCEPTION_EXECUTE_HANDLER){error=9;}
  if(error)break;fwrite(point,4,4,output);fwrite(direction,4,4,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=10;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=11;}
 if(ferror(input)||ferror(output))error=12;int ci=fclose(input),co=fclose(output);if(ci||co)error=12;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("contact_reduction_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=11);return (int)error;
}
