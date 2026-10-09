/* Whole B77770, owned owner/contact pools, no helpers or imports executed. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef struct {unsigned identity;unsigned char bytes[0x420];} Owner;
static void *owner_for(Owner *owners,unsigned n,unsigned id){for(unsigned i=0;i<n;++i)if(owners[i].identity==id)return owners[i].bytes;return NULL;}
static unsigned collect(void *function,void *manager,void **output,unsigned ignored_capacity){
 unsigned result;
 __asm {mov ecx,manager
  push ignored_capacity
  push output
  mov eax,function
  call eax
  mov result,eax
 }
 return result;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31434e4f||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  unsigned h[4];Owner owners[64]={0};unsigned char manager[0xb8]={0},contacts[1024][0x80]={0};void *pointers[1024],*result[64]={0};
  if(fread(h,4,4,input)!=4||h[0]>64||h[1]>512||h[2]>512){error=7;break;}
  for(unsigned i=0;i<h[0];++i){unsigned w[2];if(fread(w,4,2,input)!=2||!w[0]||w[1]>1||owner_for(owners,i,w[0])){error=8;break;}owners[i].identity=w[0];*(unsigned *)(owners[i].bytes+0x418)=w[1];}
  if(error)break;
  for(unsigned i=0;i<h[1]+h[2];++i){unsigned w[3];if(fread(w,4,3,input)!=3||w[2]>1){error=8;break;}void *a=owner_for(owners,h[0],w[0]),*b=owner_for(owners,h[0],w[1]);
   if(!a||(i>=h[1]&&(!b||w[2]!=0))){error=8;break;}*(void **)(contacts[i]+4)=a;*(void **)(contacts[i]+8)=b;contacts[i][0xc]=(unsigned char)w[2];pointers[i]=contacts[i];
  }
  if(error)break;*(void **)(manager+0x98)=pointers;*(void **)(manager+0xa0)=pointers+h[1];*(unsigned *)(manager+0xac)=h[1];*(unsigned *)(manager+0xb4)=h[2];
  unsigned n=0;__try{n=collect(image+0xb77770-0x400000,manager,result,h[3]);}__except(EXCEPTION_EXECUTE_HANDLER){error=9;}
  if(error)break;if(n>h[0]){error=10;break;}fwrite(&n,4,1,output);
  for(unsigned i=0;i<n;++i){unsigned j;for(j=0;j<h[0]&&result[i]!=owners[j].bytes;++j){}if(j==h[0]){error=10;break;}fwrite(&owners[j].identity,4,1,output);}
  if(error)break;++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("owner_collection_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=12);return (int)error;
}
