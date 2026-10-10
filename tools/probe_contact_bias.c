/* Complete original B0CE10/B0CEC0, supplied correction row, no stubs/imports. */
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
__declspec(noinline) static void invoke(void *function,void *contact,void *context){
 __asm { mov ecx,contact
         push context
         call function }
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 unsigned char *global=image+0xe51d40-0x400000;DWORD previous=0;unsigned saved=0;
 if(!VirtualProtect(global,4,PAGE_READWRITE,&previous)){free(code);VirtualFree(image,0,MEM_RELEASE);return 5;}memcpy(&saved,global,4);
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 6;
 if(fseek(input,0,SEEK_END)||ftell(input)>16*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31494243||fread(&total,4,1,input)!=1||!total||total>16384){error=7;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;++i){
  unsigned h[3];float values[4];__declspec(align(16)) unsigned char row[192],contact[128]={0},context[80]={0};
  if(fread(h,4,3,input)!=3||fread(values,4,4,input)!=4||fread(row,1,192,input)!=192||h[0]>1||h[1]>1||h[2]>1){error=8;break;}
  for(unsigned j=0;j<4;++j)if(!isfinite(values[j]))error=8;
  if(error)break;
  if(!h[1]){volatile float delta=values[0]-values[1];volatile float bias=delta*values[2];if(!isfinite(bias)){error=8;break;}}
  contact[0xe]=(unsigned char)h[1];memcpy(contact+0x38,values,4);memcpy(context+0xc,values+1,4);memcpy(context+8,values+2,4);memcpy(global,values+3,4);
  context[0x20]=(unsigned char)h[2];unsigned offset=h[0]?0x40:0x38;*(void **)(context+offset)=row;
  void *function=image+(h[0]?0xb0ce10:0xb0cec0)-0x400000;
  __try{invoke(function,contact,context);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"contact_bias_fault=%08lx\n",GetExceptionCode());error=9;}
  if(error)break;unsigned advance=(unsigned)(*(unsigned char **)(context+offset)-row),active=context[0x20];
  if(advance!=192||active>1){error=10;break;}fwrite(&advance,4,1,output);fwrite(&active,4,1,output);fwrite(row,1,192,output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 memcpy(global,&saved,4);DWORD ignored;if(!VirtualProtect(global,4,previous,&ignored))error=12;
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(ferror(input)||ferror(output))error=14;int ci=fclose(input),co=fclose(output);if(ci||co)error=14;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("contact_bias_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
