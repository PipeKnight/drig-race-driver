/* Whole original B77490; bounded owned contacts/owners/pending pairs, no stubs. */
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
typedef void (__stdcall *Consume)(void *,void **,int,float *,void *,int *);
typedef struct { unsigned identity; unsigned char bytes[0x420]; } Owner;
typedef struct { unsigned identity; unsigned char bytes[0x80]; } Contact;
typedef struct { void *contact; float value; } Pair;
_Static_assert(sizeof(Pair)==8,"x86 pair layout");
static void *owner_for(Owner *owners,unsigned count,unsigned identity){
 if(!identity)return NULL;
 for(unsigned i=0;i<count;++i)if(owners[i].identity==identity)return owners[i].bytes;
 return NULL;
}
static void *contact_for(Contact *contacts,unsigned count,unsigned identity){
 for(unsigned i=0;i<count;++i)if(contacts[i].identity==identity)return contacts[i].bytes;
 return NULL;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0,steps=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31435352||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  unsigned h[4];Owner owners[64]={0};Contact contacts[512]={0};Pair pairs[240]={0};
  if(fread(h,4,4,input)!=4||!h[0]||h[0]>64||h[1]>512||h[2]>240||!h[3]||h[3]>8){error=7;break;}
  for(unsigned i=0;i<h[0];++i){unsigned w[2];if(fread(w,4,2,input)!=2||!w[0]||w[1]>1||owner_for(owners,i,w[0])){error=8;break;}owners[i].identity=w[0];*(unsigned *)(owners[i].bytes+0x41c)=w[1];}
  if(error)break;
  for(unsigned i=0;i<h[1];++i){unsigned w[5];if(fread(w,4,5,input)!=5||!w[0]||w[3]>1||contact_for(contacts,i,w[0])){error=8;break;}
   void *a=owner_for(owners,h[0],w[1]),*b=owner_for(owners,h[0],w[2]);float value;memcpy(&value,w+4,4);
   if((w[1]&&!a)||(w[2]&&!b)||!isfinite(value)){error=8;break;}
   contacts[i].identity=w[0];*(void **)(contacts[i].bytes+4)=a;*(void **)(contacts[i].bytes+8)=b;contacts[i].bytes[0xc]=(unsigned char)w[3];memcpy(contacts[i].bytes+0x38,&value,4);
  }
  if(error)break;
  for(unsigned i=0;i<h[2];++i){unsigned identity;float value;if(fread(&identity,4,1,input)!=1||fread(&value,4,1,input)!=1||!isfinite(value)||!(pairs[i].contact=contact_for(contacts,h[1],identity))){error=8;break;}pairs[i].value=value;}
  if(error)break;int pending=(int)h[2];fwrite(h+3,4,1,output);
  for(unsigned pass=0;pass<h[3];++pass){
   unsigned ph[2];void *selected[80],*owner;float response[80];
   if(fread(ph,4,2,input)!=2||ph[1]>80||!(owner=owner_for(owners,h[0],ph[0]))){error=8;break;}
   for(unsigned i=0;i<ph[1];++i){unsigned index;if(fread(&index,4,1,input)!=1||fread(response+i,4,1,input)!=1||index>=h[1]||!isfinite(response[i])){error=8;break;}selected[i]=contacts[index].bytes;}
   if(error)break;Consume consume=(Consume)(image+0xb77490-0x400000);
   __try{consume(owner,selected,(int)ph[1],response,pairs,&pending);}__except(EXCEPTION_EXECUTE_HANDLER){error=9;}
   if(error)break;if(pending<0||pending>240){error=10;break;}
   fwrite(h+1,4,1,output);fwrite(ph+1,4,1,output);fwrite(&pending,4,1,output);
   for(unsigned i=0;i<h[1];++i){float value;memcpy(&value,contacts[i].bytes+0x38,4);if(!isfinite(value)){error=10;break;}fwrite(&value,4,1,output);}
   for(unsigned i=0;i<ph[1];++i)if(!isfinite(response[i]))error=10;
   if(error)break;fwrite(response,4,ph[1],output);
   for(int i=0;i<pending;++i){unsigned j;for(j=0;j<h[1]&&pairs[i].contact!=contacts[j].bytes;++j){}if(j==h[1]||!isfinite(pairs[i].value)){error=10;break;}fwrite(&contacts[j].identity,4,1,output);fwrite(&pairs[i].value,4,1,output);}
   if(error)break;++steps;
  }
  if(error)break;++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("response_consumption_cases=%u steps=%u error=%u code_unchanged=%u no_imports=1\n",cases,steps,error,error!=12);return (int)error;
}
