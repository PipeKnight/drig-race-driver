/* Whole original9A15D0, supplied linked pose, no stubs/imports/code writes. */
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
typedef void (__cdecl *Inverse)(void *,void *);
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 DWORD protection,ignored;unsigned char initial[4096];memcpy(initial,image+0xe5a000-0x400000,4096);
 if(!VirtualProtect(image+0xe5a000-0x400000,4096,PAGE_READWRITE,&protection)||protection!=PAGE_READONLY)return 4;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 5;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 6;
 if(fseek(input,0,SEEK_END)||ftell(input)>8+1024*64||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31494349||fread(&total,4,1,input)!=1||!total||total>1024){error=7;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  __declspec(align(16)) unsigned char linked[0x210]={0};__declspec(align(16)) float result[16];
  if(fread(linked+0x1d0,1,64,input)!=64){error=8;break;}
  for(unsigned i=0;i<16;++i)if(!isfinite(((float *)(linked+0x1d0))[i]))error=8;if(error)break;
  memcpy(image+0xe5a000-0x400000,initial,4096);Inverse inverse=(Inverse)(image+0x9a15d0-0x400000);
  __try{inverse(linked,result);}__except(EXCEPTION_EXECUTE_HANDLER){error=9;}if(error)break;
  for(unsigned i=0;i<16;++i)if(!isfinite(result[i]))error=9;if(error)break;
  fwrite(result,4,16,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=10;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=11;}
 memcpy(image+0xe5a000-0x400000,initial,4096);if(!VirtualProtect(image+0xe5a000-0x400000,4096,protection,&ignored))error=12;
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("contact_inverse_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=11);return (int)error;
}
