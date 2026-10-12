/* Whole unchanged source suspension context on an owned canonical car. */
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
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 17;}
 memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) unsigned char owned[32+0x2600+32];unsigned char *body=owned+32;memset(owned,0xcd,sizeof(owned));
 unsigned error=0,protected=0,tag=0,restored=1;DWORD protection=0;unsigned char saved[4096];FILE *input=NULL;
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
 int valid=fread(&tag,4,1,input)==1&&fread(body,1,0x2600,input)==0x2600&&fgetc(input)==EOF;fclose(input);input=NULL;
 if(!valid||tag!=0x31305443){error=6;goto done;}
 if(get(body,0x9a4)||get(body,0x9a8)!=4){error=7;goto done;}
 const unsigned angles[]={0x103c,0x1040,0x1044,0x1048};
 for(unsigned i=0;i<4;i++){float value;memcpy(&value,body+angles[i],4);if(!isfinite(value)||fabsf(value)>1.f)error=8;}
 for(unsigned i=0x14e0;i<0x1520;i+=4){float value;memcpy(&value,body+i,4);if(!isfinite(value))error=8;}
 if(error)goto done;
 unsigned char *page=image+0xe58000-0x400000;memcpy(saved,page,4096);
 if(!VirtualProtect(page,4096,PAGE_READWRITE,&protection)){error=9;goto done;}protected=1;
 put(body,0x9a4,(unsigned)(uintptr_t)body);
 __try{((void (__fastcall *)(void *,void *))(image+0x9c10b0-0x400000))(body+0x3d0,NULL);}
 __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"context_fault=%08lx\n",GetExceptionCode());error=10;}
 if(!error){
  if(get(body,0x9a4)!=(unsigned)(uintptr_t)body)error=11;put(body,0x9a4,0);
  for(unsigned i=0;i<4;i++){unsigned o=0x3d0+i*0x170+0x178,token=0x1990+i*0x150;if(get(body,o)!=(unsigned)(uintptr_t)(body+token))error=11;put(body,o,token);}
 }
 for(unsigned i=0;i<32;i++)if(owned[i]!=0xcd||owned[32+0x2600+i]!=0xcd)error=12;
done:
 if(protected){DWORD ignored;unsigned char *restore_page=image+0xe58000-0x400000;memcpy(restore_page,saved,4096);if(!VirtualProtect(restore_page,4096,protection,&ignored)||memcmp(restore_page,saved,4096)){error=16;restored=0;}}
 unsigned code_unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))code_unchanged=0;
 if(!code_unchanged)error=18;
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=14;else{DWORD n=0;if(!WriteFile(output,body,0x2600,&n,NULL)||n!=0x2600)error=15;CloseHandle(output);}}
 if(input)fclose(input);free(snapshot);VirtualFree(image,0,MEM_RELEASE);
 printf("context_bytes=9728 code_unchanged=%u code_replacements=0 globals_restored=%u error=%u\n",code_unchanged,restored,error);return (int)error;
}
