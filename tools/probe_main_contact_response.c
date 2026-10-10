/* Whole main preparation B77CB0/B79EE0/B78D30/770AB0 and all B791B0 spans. */
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
__declspec(noinline) static void two_args(void *fn,void *manager,unsigned first,unsigned second){
 __asm {mov ecx,manager
        push second
        push first
        call fn}
}
__declspec(noinline) static void one_arg(void *fn,void *manager,unsigned first){
 __asm {mov ecx,manager
        push first
        call fn}
}
static unsigned get(const void *p,unsigned offset){unsigned v;memcpy(&v,(const unsigned char *)p+offset,4);return v;}
static void put(void *p,unsigned offset,unsigned v){memcpy((unsigned char *)p+offset,&v,4);}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float f;memcpy(&f,(const unsigned char *)p+4*i,4);if(!isfinite(f))return 0;}return 1;}
static int finite_row(const unsigned char *r){const unsigned offsets[]={0x80,0x84,0x88,0x8c,0x90,0x94,0x98,0x9c,0xac,0xb0};if(!finite_words(r,32))return 0;for(unsigned i=0;i<10;i++)if(!finite_words(r+offsets[i],1))return 0;return 1;}
static void publish(FILE *output,unsigned char rows[256][192],unsigned n,unsigned normal,const unsigned *references){
 for(unsigned i=0;i<n;i++){unsigned char row[192];memcpy(row,rows[i],192);if(i>=normal)put(row,0xa8,references[i]);fwrite(row,1,192,output);}
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 unsigned dispatch=get(image,0xfb1fa0-0x400000);
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x3152434d||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;c++){
  unsigned h[7],references[256]={0};float parameters[3];
  __declspec(align(16)) unsigned char rows[256][192],manager[0xd0]={0};
  __declspec(align(16)) float workspace[64][8],inertia[64][16],masses[64];
  unsigned char manager_before[0xd0];float inertia_before[64][16],masses_before[64];
  float body_wire[25];
  if(fread(h,4,7,input)!=7||fread(parameters,4,3,input)!=3||h[0]>256||h[1]>64||h[2]>h[3]||h[3]>h[4]||h[4]>h[5]||h[5]!=h[0]||h[6]>32||!finite_words(parameters,3)||fread(rows,192,h[0],input)!=h[0]){error=7;break;}
  for(unsigned i=0;i<h[1];i++){
   if(fread(body_wire,4,25,input)!=25||!finite_words(body_wire,25)){error=8;break;}
   memcpy(workspace[i],body_wire,32);masses[i]=body_wire[8];memcpy(inertia[i],body_wire+9,64);
  }
  for(unsigned i=0;!error&&i<h[0];i++)if(!finite_row(rows[i])||*(float *)(rows[i]+0x88)>*(float *)(rows[i]+0x8c))error=8;
  unsigned start=0;
  for(unsigned span=0;!error&&span<4;span++){
   unsigned end=h[2+span],cursor=start;
   while(cursor<end){
    unsigned length=get(rows[cursor],0xb4),a=get(rows[cursor],0xa0),b=get(rows[cursor],0xa4);
    if(!length||length>end-cursor||a>=h[1]||(!(span%2)&&b>=h[1])){error=8;break;}
    for(unsigned i=cursor;i<cursor+length;i++){
     if(get(rows[i],0xa0)!=a||(!(span%2)&&get(rows[i],0xa4)!=b)){error=8;break;}
     if(span>=2){unsigned ref=get(rows[i],0xa8);if(ref>=h[3]){error=8;break;}references[i]=ref;put(rows[i],0xa8,(unsigned)(uintptr_t)(rows[ref]+0x98));}
    }
    if(error)break;cursor+=length;
   }
   start=end;
  }
  if(error)break;
  put(manager,0x80,(unsigned)(uintptr_t)workspace);put(manager,0x84,(unsigned)(uintptr_t)inertia);put(manager,0x88,(unsigned)(uintptr_t)masses);put(manager,0x8c,(unsigned)(uintptr_t)rows);
  put(manager,0xc4,h[2]);put(manager,0xbc,h[3]);put(manager,0xc8,h[4]);put(manager,0xc0,h[5]);
  memcpy(manager_before,manager,sizeof(manager));memcpy(inertia_before,inertia,h[1]*64);memcpy(masses_before,masses,h[1]*4);
  fwrite(h,4,2,output);
  __try{
   two_args(image+0xb77cb0-0x400000,manager,bits(parameters[0]),bits(parameters[1]));
   publish(output,rows,h[0],h[3],references);
   one_arg(image+0xb79ee0-0x400000,manager,bits(parameters[2]));
   for(unsigned i=0;i<h[0];i++)if(!finite_row(rows[i]))error=10;
   if(!error){publish(output,rows,h[0],h[3],references);
    two_args(image+0xb791b0-0x400000,manager,h[1],h[6]);
    for(unsigned i=0;i<h[0];i++)if(!finite_row(rows[i]))error=10;
    if(!finite_words(workspace,h[1]*8))error=10;
    if(!error){publish(output,rows,h[0],h[3],references);fwrite(workspace,32,h[1],output);}
   }
  }__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"main_contact_response_fault=%08lx case=%u\n",GetExceptionCode(),c);error=9;}
  if(memcmp(manager_before,manager,sizeof(manager))||memcmp(inertia_before,inertia,h[1]*64)||memcmp(masses_before,masses,h[1]*4))error=10;
  if(error)break;++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("main_contact_response_cases=%u error=%u code_unchanged=%u no_imports=1 original_memset_dispatch=%u\n",cases,error,error!=12,dispatch);return (int)error;
}
