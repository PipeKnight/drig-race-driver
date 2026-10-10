/* Complete linked setters -> B13E10 -> B13B00 -> linked getters, authored order.
   No entry/import execution, helper stubs, executable writes or live injection. */
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
__declspec(noinline) static void unary(void *function,void *wrapper,void *vector){
 __asm {mov ecx,wrapper
        push vector
        call function}
}
__declspec(noinline) static void set_pose(void *function,void *wrapper,void *pose,unsigned notify){
 __asm {mov ecx,wrapper
        push notify
        push pose
        call function}
}
__declspec(noinline) static void advance(void *function,void *body,unsigned delta){
 __asm {mov ecx,body
        push 0
        push body
        push delta
        call function}
}
__declspec(noinline) static void correct(void *function,void *body,void *linear,void *angular){
 __asm {mov ecx,body
        push angular
        push linear
        call function}
}
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float v;memcpy(&v,(const unsigned char *)p+4*i,4);if(!isfinite(v))return 0;}return 1;}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"vehicle_bridge_fault=%08lx eip=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot)return 4;memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<4;i++){if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protections+i)){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31425656||!read(input,&total,4)||!total||total>128))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;c++){
  unsigned h[6],selectors[64];__declspec(align(16)) unsigned char body[0x430],vehicle[0x2600],shapes[64][0x220],group[0x70]={0},wrapper[16]={0};__declspec(align(16)) float correction[8];
  if(!read(input,h,24)||h[0]>64||h[1]>1||h[2]>1||h[3]>1||h[4]>1||!read(input,body,0x424)||!read(input,vehicle,0x2600)||!read(input,correction,32)||!read(input,shapes,h[0]*0x220)){error=8;break;}
  float delta;memcpy(&delta,h+5,4);
  const unsigned pointers[]={0x328,0x32c,0x334,0x338,0x33c,0x340};
  for(unsigned i=0;i<6;i++)if(get(body,pointers[i]))error=8;
  if((h[1]&&get(body,0x400)==0xffffffff)||(h[2]&&(!isfinite(delta)||delta<=0||delta>.25f))||!finite_words(body+0x90,16)||!finite_words(body+0x150,16)||!finite_words(body+0x230,4)||!finite_words(vehicle+0x15c0,8)||!finite_words(correction,8))error=8;
  for(unsigned r=0;r<4;r++)if(!finite_words(vehicle+0x15e0+r*16,3))error=8;
  float angular=0;for(unsigned i=4;i<8;i++)angular+=correction[i]*correction[i];if(h[3]&&(!isfinite(angular)||angular>4096.f))error=8;
  float spin=0;for(unsigned i=0;i<4;i++){float v;memcpy(&v,vehicle+0x15d0+i*4,4);spin+=v*v;}if(h[2]&&(!isfinite(spin)||sqrtf(spin)*delta>64.f))error=8;
  for(unsigned i=0;i<h[0];i++){selectors[i]=get(shapes[i],0x170);if(get(shapes[i],0xe0)||selectors[i]>2||shapes[i][0x214]>1||!finite_words(shapes[i]+0xf0,16))error=8;}
  if(error)break;
  put(wrapper,8,(unsigned)(uintptr_t)body);put(group,0x28,h[4]?(unsigned)(uintptr_t)body:0);put(body,0x32c,(unsigned)(uintptr_t)group);
  put(body,0x328,h[0]?(unsigned)(uintptr_t)shapes:0);
  for(unsigned i=0;i<4;i++)put(body,0x334+i*4,(unsigned)(uintptr_t)(body+(i?0x180+i*16:0x1c0)));
  for(unsigned i=0;i<h[0];i++){put(shapes[i],0xe0,i+1<h[0]?(unsigned)(uintptr_t)shapes[i+1]:0);put(shapes[i],0x170,selectors[i]?(unsigned)(uintptr_t)(shapes[i]+(selectors[i]==1?0x130:0xf0)):0);}
  fwrite(h,4,6,output);
  for(unsigned stage=0;stage<3;stage++){
   __try{
    if(!stage){set_pose(image+0xa696f0-0x400000,wrapper,vehicle+0x15e0,h[1]);unary(image+0xa69840-0x400000,wrapper,vehicle+0x15c0);unary(image+0xa69860-0x400000,wrapper,vehicle+0x15d0);}
    if(stage==1&&h[2])advance(image+0xb13e10-0x400000,body,h[5]);
    if(stage==2&&h[3])correct(image+0xb13b00-0x400000,body,correction,correction+4);
    unary(image+0xa69800-0x400000,wrapper,vehicle+0x15c0);unary(image+0xa69820-0x400000,wrapper,vehicle+0x15d0);unary(image+0xa69630-0x400000,wrapper,vehicle+0x15e0);
   }__except(fault(GetExceptionInformation())){error=9;}
   if(error)break;
   if(!finite_words(body+0x90,16)||!finite_words(body+0xd0,32)||!finite_words(body+0x190,32))error=10;
   for(unsigned i=0;i<h[0];i++)if(!finite_words(shapes[i]+0x130,16))error=10;
   if(error)break;
   for(unsigned i=0;i<6;i++)put(body,pointers[i],0);
   for(unsigned i=0;i<h[0];i++){put(shapes[i],0xe0,0);put(shapes[i],0x170,selectors[i]);}
   fwrite(body,1,0x424,output);fwrite(vehicle,1,0x2600,output);fwrite(shapes,0x220,h[0],output);
   put(body,0x32c,(unsigned)(uintptr_t)group);put(body,0x328,h[0]?(unsigned)(uintptr_t)shapes:0);
   for(unsigned i=0;i<4;i++)put(body,0x334+i*4,(unsigned)(uintptr_t)(body+(i?0x180+i*16:0x1c0)));
   for(unsigned i=0;i<h[0];i++){put(shapes[i],0xe0,i+1<h[0]?(unsigned)(uintptr_t)shapes[i+1]:0);put(shapes[i],0x170,selectors[i]?(unsigned)(uintptr_t)(shapes[i]+(selectors[i]==1?0x130:0xf0)):0);}
  }
  if(error)break;++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(snapshot+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 for(unsigned i=0;i<protected;i++){DWORD ignored;memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,0x1000);if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=13;}
 if(input){if(ferror(input)||fclose(input))error=14;}if(output){if(ferror(output)||fclose(output))error=14;}
 if(error&&output)_wremove(argv[3]);free(snapshot);VirtualFree(image,0,MEM_RELEASE);
 printf("vehicle_bridge_cases=%u error=%u code_unchanged=%u globals_restored=1\n",cases,error,error!=12);return (int)error;
}
