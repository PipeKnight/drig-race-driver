/* Whole unchanged original AD6C40, owned cold active bodies/impact pool, CG01. */
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
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static unsigned ptr(const void *p){return (unsigned)(uintptr_t)p;}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float v;memcpy(&v,(const unsigned char *)p+4*i,4);if(!isfinite(v))return 0;}return 1;}
static unsigned token(unsigned address,unsigned char *base,unsigned stride,unsigned count,unsigned *error){
 if(!address)return 0;
 for(unsigned i=0;i<count;i++)if(address==ptr(base+i*stride))return i+1;
 *error=10;return 0;
}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"contact_groups_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 4;}memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,magic=0,total=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31304743||!read(input,&total,4)||!total||total>1024))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;c++){
  unsigned h[3],order[16],seen[16]={0},manager_ptrs[1],state[11];
  __declspec(align(16)) unsigned char world[0x810]={0},producer[0x940]={0},manager[0x120]={0},bodies[16][0x430]={0},contacts[64][0x80]={0};
  unsigned char world_before[0x810],producer_before[0x940],manager_before[0x120];
  if(!read(input,h,12)||!h[0]||h[0]>16||h[1]>64||h[2]>1||!read(input,order,h[0]*4)){error=8;break;}
  for(unsigned j=0;j<h[0];j++){if(order[j]>=h[0]||seen[order[j]])error=8;else seen[order[j]]=1;}
  for(unsigned j=0;!error&&j<h[0];j++){
   unsigned char *b=bodies[j];
   if(!read(input,b,0x424)||get(b,0x334)!=0x1c0||get(b,0x400)!=h[2]||b[0x2c]>1||b[0x2d]||b[0x412]||b[0x413]||!finite_words(b+0x1c0,4)){error=8;break;}
   const unsigned pointers[]={0,4,8,0xc,0x10,0x14,0x18,0x1c,0x2f0,0x2f4,0x2fc,0x300,0x304,0x308,0x30c,0x318,0x320,0x324,0x328,0x32c,0x418,0x41c,0x420};
   for(unsigned k=0;k<sizeof(pointers)/sizeof(*pointers);k++)if(get(b,pointers[k]))error=8;
   put(b,0x32c,ptr(world));put(b,0x334,ptr(b+0x1c0));
  }
  for(unsigned j=0;!error&&j<h[1];j++){
   unsigned char *r=contacts[j];
   if(!read(input,r,128)){error=8;break;}unsigned a=get(r,4),b=get(r,8);
   if(!a||a>h[0]||b>h[0]||r[0xc]>1||(!r[0xc]&&(!b||a==b))||get(r,0x10)||r[0xf]!=1||r[0x73]||get(r,0x28)||get(r,0x2c)||!finite_words(r+0x50,8)){error=8;break;}
   for(unsigned k=0;k<4;k++){float x,y;memcpy(&x,bodies[a-1]+0x1c0+k*4,4);memcpy(&y,r+0x50+k*4,4);if(!isfinite(x+y))error=8;}
   put(r,4,ptr(bodies[a-1]));put(r,8,b?ptr(bodies[b-1]):0);
  }
  if(error)break;
  for(unsigned j=0;j<h[0];j++){unsigned char *b=bodies[order[j]];put(b,0x300,j+1<h[0]?ptr(bodies[order[j+1]]):0);put(b,0x304,j?ptr(bodies[order[j-1]]):0);}
  manager_ptrs[0]=ptr(manager);put(manager,0,ptr(image+0xd42654-0x400000));
  put(producer,0x91c,ptr(contacts));put(producer,0x924,h[1]);
  put(world,4,1);put(world,0x10,ptr(manager_ptrs));put(world,0x18,ptr(producer));
  put(world,0x3c+h[2]*4,ptr(bodies[order[0]]));put(world,0x44+h[2]*4,ptr(bodies[order[h[0]-1]]));put(world,0x60,h[2]);put(world,0x64,1-h[2]);
  memcpy(world_before,world,sizeof(world));memcpy(producer_before,producer,sizeof(producer));memcpy(manager_before,manager,sizeof(manager));
  __try{((void (__fastcall *)(void *,void *))(image+0xad6c40-0x400000))(world,NULL);}
  __except(fault(GetExceptionInformation())){error=9;fprintf(stderr,"case=%u\n",c);}
  if(error)break;
  const unsigned changed[]={0x38,0x3c,0x40,0x44,0x48,0x50,0x60,0x64,0x7f8,0x7fc};
  for(unsigned j=0;j<sizeof(changed)/sizeof(*changed);j++)memcpy(world_before+changed[j],world+changed[j],4);
  memcpy(manager_before+0x74,manager+0x74,4);
  if(memcmp(world_before,world,sizeof(world))||memcmp(producer_before,producer,sizeof(producer))||memcmp(manager_before,manager,sizeof(manager))){error=10;break;}
  state[0]=token(get(world,0x50),bodies[0],0x430,h[0],&error);state[1]=token(get(manager,0x74),bodies[0],0x430,h[0],&error);
  state[2]=get(world,0x7fc);state[3]=get(world,0x7f8);state[4]=token(get(world,0x38),bodies[0],0x430,h[0],&error);state[5]=get(world,0x60);state[6]=get(world,0x64);
  for(unsigned j=0;j<4;j++)state[7+j]=token(get(world,0x3c+j*4),bodies[0],0x430,h[0],&error);
  for(unsigned j=0;j<h[0];j++){
   unsigned char *b=bodies[j];const unsigned body_links[]={8,0xc,0x10,0x14,0x18,0x1c,0x2f0,0x2f4,0x2fc,0x300,0x304};
   for(unsigned k=0;k<sizeof(body_links)/sizeof(*body_links);k++)put(b,body_links[k],token(get(b,body_links[k]),bodies[0],0x430,h[0],&error));
   for(unsigned k=0;k<2;k++)put(b,k*4,token(get(b,k*4),contacts[0],0x80,h[1],&error));
   if(get(b,0x32c)!=ptr(world)||get(b,0x334)!=ptr(b+0x1c0))error=10;
   put(b,0x32c,0);put(b,0x334,0x1c0);
  }
  for(unsigned j=0;j<h[1];j++){
   unsigned char *r=contacts[j];put(r,4,token(get(r,4),bodies[0],0x430,h[0],&error));put(r,8,token(get(r,8),bodies[0],0x430,h[0],&error));put(r,0x10,token(get(r,0x10),contacts[0],0x80,h[1],&error));
  }
  if(error)break;
  fwrite(state,4,11,output);for(unsigned j=0;j<h[0];j++)fwrite(bodies[j],1,0x424,output);fwrite(contacts,128,h[1],output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t n=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(snapshot+sections[i].VirtualAddress,image+sections[i].VirtualAddress,n))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;
 if(input&&fclose(input))error=14;if(output&&fclose(output))error=14;
 if(error&&output)_wremove(argv[3]);free(snapshot);VirtualFree(image,0,MEM_RELEASE);
 printf("contact_groups_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
