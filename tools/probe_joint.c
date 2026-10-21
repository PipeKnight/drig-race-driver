/* Original joint (NeMouse) constructor AEFEB0 (mode 0), then prepare AF4890
   with moved body poses (mode 1), then the rows AF71C0/AF9430 into
   pre-initialised row pools (mode 2), then the write-back AFB280/AFB800 after
   supplied row impulses (mode 3), then the correction rows AEF930/AEFBB0
   after B7B5E0's row reset (mode 4), JNT1. Supplied descriptor, body states
   and row context; world/body list links isolated, no island merging. */
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
__declspec(noinline) static void prepare(void *function,void *joint){
 __asm { mov ecx,joint
         call function }
}
__declspec(noinline) static void rows(void *function,void *joint,void *context){
 __asm { mov ecx,joint
         push context
         call function }
}
__declspec(noinline) static void construct(void *function,void *joint,void *descriptor){
 __asm { mov ecx,joint
         push descriptor
         call function }
}
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static unsigned char *base;
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"joint_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,(unsigned long)(p->ContextRecord->Eip-(DWORD)(uintptr_t)base+0x400000),p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
/* Pointer-valued joint words: vtable, bodies, world, list links. */
static const unsigned pointers[]={0x0,0x4,0x8,0x28,0x2c,0x30,0x34,0x38,0x3c,0x40,0x44};
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;base=image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 unsigned pages[4]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4]={0};unsigned char saved[4][4096];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<4;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,&protections[i])){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0,first=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>64*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31544e4a||!read(input,&total,4)||!total||total>16384))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  unsigned h[4];
  __declspec(align(16)) unsigned char descriptor[0x170]={0},bodies[2][0x430]={0},world[0x800]={0},joint[0x340]={0};
  __declspec(align(16)) unsigned char moved[2][0x40],context[0x60]={0},pools[2][16][0xc0];
  unsigned tail[8],solved[1+2*16*2+3];
  if(!read(input,h,16)||h[0]>4||h[0]!=(i?first:(first=h[0]))||h[1]>3||h[3]>1||!read(input,descriptor,0x168)||!read(input,bodies[0],0x424)||!read(input,bodies[1],0x424)||!read(input,moved,0x80)||!read(input,tail,32)||tail[6]>2||(h[0]>=3&&!read(input,solved,(h[0]==4?3:0)*4+(1+2*16*2)*4))){error=8;break;}
  for(unsigned b=0;b<2;b++){put(bodies[b],0x334,(unsigned)(uintptr_t)(bodies[b]+0x1c0));put(bodies[b],0x320,0);put(bodies[b],0x324,0);put(bodies[b],0x54,0);put(bodies[b],0x400,0xffffffff);}
  put(world,0x7d0,h[2]);
  put(descriptor,0,(unsigned)(uintptr_t)world);
  put(descriptor,4,(unsigned)(uintptr_t)bodies[0]);
  put(descriptor,8,(h[1]&1)?(unsigned)(uintptr_t)bodies[1]:0);
  descriptor[0xc]=(unsigned char)((h[1]>>1)&1);
  memset(joint,0xcd,sizeof joint);
  __try{construct(image+0xaefeb0-0x400000,joint,descriptor);}__except(fault(GetExceptionInformation())){error=9;}
  if(error)break;
  if(h[0]>=1){
   for(unsigned b=0;b<2;b++)memcpy(bodies[b]+0x190,moved[b],0x40);
   joint[0x32d]=(unsigned char)h[3];
   __try{prepare(image+0xaf4890-0x400000,joint);}__except(fault(GetExceptionInformation())){error=9;}
   if(error)break;
  }
  if(h[0]>=2){
   /* B7ACC0's row initialisation over a deterministic fill. */
   for(unsigned q=0;q<2;q++)for(unsigned r=0;r<16;r++){
    for(unsigned w=0;w<48;w++)put(pools[q][r],w*4,tail[7]^(q*0x10000+r*48+w));
    put(pools[q][r],32*4,0);put(pools[q][r],33*4,tail[3]);put(pools[q][r],34*4,0xff7fffff);put(pools[q][r],35*4,0x7f7fffff);
    put(pools[q][r],42*4,0);put(pools[q][r],40*4,0xffffffff);put(pools[q][r],41*4,0xffffffff);put(pools[q][r],45*4,0);
   }
   put(context,4,tail[0]);put(context,8,tail[1]);put(context,0x14,tail[2]);
   put(joint,0x1c,tail[4]);put(joint,0x20,tail[5]);
   unsigned start=tail[6]?1:0;
   put(context,0x38,(unsigned)(uintptr_t)pools[0][start]);put(context,0x40,(unsigned)(uintptr_t)pools[1][start]);
   if(tail[6]){
    unsigned other=tail[6]==2?tail[4]+1:tail[4];
    for(unsigned q=0;q<2;q++){put(pools[q][0],40*4,other);put(pools[q][0],41*4,tail[5]);}
    put(context,0x48,(unsigned)(uintptr_t)pools[0][0]);put(context,0x50,(unsigned)(uintptr_t)pools[1][0]);
    put(context,0x24,1);put(context,0x28,1);
   }
   int two=(h[1]&1)&&!(h[1]&2);
   __try{rows(image+(two?0xaf71c0:0xaf9430)-0x400000,joint,context);}__except(fault(GetExceptionInformation())){error=9;}
   if(error)break;
   if(h[0]>=3){
    /* After the solve: the rows' impulses (37) and denominators (43). */
    put(context,0x18,solved[0]);
    for(unsigned q=0;q<2;q++)for(unsigned r=0;r<16;r++){put(pools[q][r],37*4,solved[1+(q*16+r)*2]);put(pools[q][r],43*4,solved[2+(q*16+r)*2]);}
    put(context,0x38,(unsigned)(uintptr_t)pools[0][start]);put(context,0x40,(unsigned)(uintptr_t)pools[1][start]);
    __try{rows(image+(two?0xafb280:0xafb800)-0x400000,joint,context);}__except(fault(GetExceptionInformation())){error=9;}
    if(error)break;
   }
   if(h[0]==4){
    /* B7B5E0: rows reset, its context (+4 1.0, +8 scale, +0x10 threshold), then the joint. */
    const unsigned *c=solved+1+2*16*2;
    joint[0xe]=(unsigned char)c[2];
    for(unsigned q=0;q<2;q++)for(unsigned r=0;r<16;r++){put(pools[q][r],32*4,0);put(pools[q][r],36*4,0);put(pools[q][r],33*4,tail[3]);}
    put(context,4,0x3f800000);put(context,8,c[0]);put(context,0x10,c[1]);put(context,0x20,0);
    put(context,0x38,(unsigned)(uintptr_t)pools[0][start]);put(context,0x40,(unsigned)(uintptr_t)pools[1][start]);
    __try{rows(image+(two?0xaef930:0xaefbb0)-0x400000,joint,context);}__except(fault(GetExceptionInformation())){error=9;}
    if(error)break;
   }
   for(unsigned q=0;q<2;q++){
    unsigned at=0x38+q*8,head=0x48+q*8;
    put(context,at,(unsigned)((get(context,at)-(unsigned)(uintptr_t)pools[q][0])/0xc0));
    put(context,head,get(context,head)?(unsigned)((get(context,head)-(unsigned)(uintptr_t)pools[q][0])/0xc0):0xffffffff);
   }
  }
  for(unsigned k=0;k<sizeof pointers/sizeof *pointers;k++)put(joint,pointers[k],0);
  fwrite(joint,1,0x330,output);fwrite(descriptor+0x10,1,0x158,output);
  if(h[0]>=2){fwrite(context,1,0x60,output);fwrite(pools,1,sizeof pools,output);}
  ++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;if(input)fclose(input);if(output)fclose(output);
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("joint_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
