/* Whole original named NED region loader on owned binary-XML views.
   Resource acquisition excluded; optional explicit host numeric CRT adapter. */
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
#include "surface_xml_probe.h"
static const char *names[]={"GEARBOX","DRIVESHAFT","TURBO","RADIATOR","EXHAUST"};
static const char *keys[]={"m_centre_x","m_centre_y","m_centre_z","m_size_x","m_size_y","m_size_z"};
static double __cdecl number_adapter(const char *s){return strtod(s,NULL);}
static int canary(const unsigned char *p,unsigned n){for(unsigned i=0;i<32;i++)if(p[i]!=0xcd||p[32+n+i]!=0xcd)return 0;return 1;}
static uintptr_t reference;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"damage_boxes_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
/* Restrict native execution to complete, unambiguous finite region attributes.
   The original getter leaves its output untouched when an attribute is absent. */
static int admitted(unsigned *doc){
 unsigned parent=~0u;
 for(unsigned i=0;i<doc[2];i++)if(!strcmp(text(doc,word((void *)(uintptr_t)(doc[3]+i*24))),"HANDLINGDAMAGE")){if(parent!=~0u)return 0;parent=i;}
 if(parent==~0u)return 1;
 unsigned *root=(unsigned *)(uintptr_t)doc[3];
 if(parent<root[5]||parent>=root[5]+root[4])return 0;
 unsigned *p=(unsigned *)(uintptr_t)(doc[3]+parent*24);
 for(unsigned k=0;k<5;k++){
  unsigned seen=0;
  for(unsigned i=p[5];i<p[5]+p[4];i++){
   unsigned *e=(unsigned *)(uintptr_t)(doc[3]+i*24);
   if(strcmp(text(doc,e[0]),names[k]))continue;
   if(seen++)return 0;
   for(unsigned j=0;j<6;j++){
    unsigned found=0;
    for(unsigned a=e[3];a<e[3]+e[2];a++){
     unsigned *pair=(unsigned *)(uintptr_t)(doc[5]+a*8);
     if(strcmp(text(doc,pair[0]),keys[j]))continue;
     const char *s=text(doc,pair[1]);char *end;double v=strtod(s,&end);
     if(found++||!*s||*end||!isfinite(v)||!isfinite((float)v)||fabs(v)>100.||(j>=3&&v<0.))return 0;
    }
    if(found!=1)return 0;
   }
  }
 }
 return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
 if(argc!=6||(wcscmp(argv[5],L"original")&&wcscmp(argv[5],L"adapted")))return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 __declspec(align(16)) float offset[4];unsigned request[6];FILE *file=NULL;
 if(_wfopen_s(&file,argv[3],L"rb")||!file)return 4;
 int valid=fread(request,4,6,file)==6&&fgetc(file)==EOF;fclose(file);
 if(!valid||request[0]!=0x31304244||request[5]>255)return 5;
 memcpy(offset,request+1,16);
 for(unsigned j=0;j<4;j++)if(!isfinite(offset[j])||fabsf(offset[j])>100.f)return 5;
 if(offset[3]!=0.f)return 5;
 unsigned doc[9]={0};unsigned char *xml=read_xml(argv[2],doc);
 if(!xml||!doc[2]||strcmp(text(doc,word((void *)(uintptr_t)doc[3])),"DAMAGE")||!admitted(doc)){free(xml);return 6;}
 unsigned xml_size=word(xml+4)+8;unsigned char *xml_saved=malloc(xml_size);
 if(!xml_saved){free(xml);return 6;}memcpy(xml_saved,xml,xml_size);
 unsigned char *image=map_reference(argv[1]);if(!image){free(xml_saved);free(xml);return 7;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned size=pe->OptionalHeader.SizeOfImage;unsigned char *snapshot=malloc(size);if(!snapshot){free(xml_saved);free(xml);VirtualFree(image,0,MEM_RELEASE);return 8;}memcpy(snapshot,image,size);
 unsigned error=0,adapted=!wcscmp(argv[5],L"adapted"),patched=0;DWORD protection=0,ignored;
 doc[0]=(unsigned)(uintptr_t)(image+0xd3d508-0x400000);unsigned *document=doc;
 unsigned doc_saved[9];memcpy(doc_saved,doc,sizeof(doc));
 __declspec(align(16)) unsigned char context[32+0x30+32],boxes[32+5*0x30+32];
 memset(context,0xcd,sizeof(context));memset(context+32,0,0x30);
 memset(boxes,0xcd,sizeof(boxes));memset(boxes+32,(int)request[5],5*0x30);
 if(adapted){unsigned char *entry=image+0xbf3b60-0x400000;
  if(!VirtualProtect(entry,5,PAGE_READWRITE,&protection)){error=9;goto done;}
  entry[0]=0xe9;unsigned relative=(unsigned)(uintptr_t)&number_adapter-(unsigned)(uintptr_t)(entry+5);memcpy(entry+1,&relative,4);patched=1;
  if(!VirtualProtect(entry,5,protection,&ignored)||!FlushInstructionCache(GetCurrentProcess(),entry,5)){error=9;goto done;}
 }
 __try{
  ((void *(__fastcall *)(void *,void *,void *,const char *))(image+0xa680e0-0x400000))(&document,NULL,context+32,"HANDLINGDAMAGE");
  for(unsigned i=0;i<5;i++)((void (__stdcall *)(const char *,void *,void *,void *))(image+0x99a5e0-0x400000))(names[i],boxes+32+i*0x30,context+32,offset);
 }__except(fault(GetExceptionInformation())){error=10;}
 if(!canary(context,0x30)||!canary(boxes,5*0x30))error=11;
 if(memcmp(xml,xml_saved,xml_size)||memcmp(offset,request+1,16)||memcmp(doc,doc_saved,sizeof(doc)))error=11;
done:
 if(patched){unsigned char *entry=image+0xbf3b60-0x400000;DWORD old;
  if(!VirtualProtect(entry,5,PAGE_READWRITE,&old))error=12;
  else{memcpy(entry,snapshot+0xbf3b60-0x400000,5);if(!VirtualProtect(entry,5,protection,&ignored)||!FlushInstructionCache(GetCurrentProcess(),entry,5))error=12;}}
 if(memcmp(image,snapshot,size))error=13;
 if(!error){HANDLE out=CreateFileW(argv[4],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
  if(out==INVALID_HANDLE_VALUE)error=14;else{DWORD written;if(!WriteFile(out,boxes+32,5*0x30,&written,NULL)||written!=5*0x30)error=15;if(!CloseHandle(out))error=16;}}
 printf("damage_boxes=5 numeric_adapters=%u restored=%u error=%u\n",adapted,error!=12&&error!=13,error);
 free(xml_saved);free(xml);free(snapshot);VirtualFree(image,0,MEM_RELEASE);return error;
}
