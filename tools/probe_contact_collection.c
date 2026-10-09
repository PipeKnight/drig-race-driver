/* Whole original B775D0 against bounded authored pointer/shape fixtures. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"
static void call_collector(void *function,void *manager,unsigned owner,void **output,int *count,int capacity){
 __asm {mov ecx,manager
  push capacity
  push count
  push output
  push owner
  mov eax,function
  call eax
 }
}
typedef struct {unsigned char contact[0x80],shape_a[0x231],shape_b[0x231];} Fixture;
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31434f43||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  unsigned h[5],tokens[80]={0};unsigned char manager[0xb8]={0};void *result[80]={0},*pointers[1024];Fixture *fixtures=NULL;
  if(fread(h,4,5,input)!=5||!h[0]||!h[1]||h[1]>80||h[2]>h[1]||h[3]>512||h[4]>512){error=7;break;}
  if(fread(tokens,4,h[2],input)!=h[2]){error=8;break;}
  for(unsigned i=0;i<h[2];++i)result[i]=(void *)(uintptr_t)tokens[i];
  unsigned count=h[3]+h[4];fixtures=calloc(count?count:1,sizeof(Fixture));if(!fixtures){error=9;break;}
  for(unsigned i=0;i<count;++i){unsigned wire[6];if(fread(wire,4,6,input)!=6||!wire[0]||wire[3]>1||wire[4]>1||wire[5]>1){error=8;break;}
   Fixture *f=fixtures+i;memcpy(f->contact,wire,12);f->contact[0xc]=(unsigned char)wire[3];
   *(void **)(f->contact+0x28)=f->shape_a;*(void **)(f->contact+0x2c)=f->shape_b;
   f->shape_a[0x230]=(unsigned char)wire[4];f->shape_b[0x230]=(unsigned char)wire[5];pointers[i]=f->contact;
  }
  if(!error){*(void **)(manager+0x98)=pointers;*(void **)(manager+0xa0)=pointers+h[3];*(unsigned *)(manager+0xac)=h[3];*(unsigned *)(manager+0xb4)=h[4];
   int written=(int)h[2];
   __try{call_collector(image+0xb775d0-0x400000,manager,h[0],result,&written,(int)h[1]);}__except(EXCEPTION_EXECUTE_HANDLER){error=10;}
   if(!error){if(written<(int)h[2]||written>(int)h[1])error=11;else{
    for(unsigned i=h[2];i<(unsigned)written;++i){unsigned j;for(j=0;j<count&&result[i]!=pointers[j];++j){}if(j==count){error=11;break;}memcpy(tokens+i,result[i],4);}
    if(!error){fwrite(&written,4,1,output);fwrite(tokens,4,(unsigned)written,output);++cases;}
   }}
  }
  free(fixtures);if(error)break;
 }
 if(!error&&fgetc(input)!=EOF)error=12;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(ferror(input)||ferror(output))error=14;int ci=fclose(input),co=fclose(output);if(ci||co)error=14;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("contact_collection_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
