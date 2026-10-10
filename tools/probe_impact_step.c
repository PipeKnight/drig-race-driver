/* Whole original B7ACC0 impact pools, owned operands, PST1. */
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
__declspec(noinline) static unsigned invoke(void *function,void *manager,unsigned *p){
 unsigned result;
 __asm { mov ecx,manager
         mov edx,p
         push [edx+16]
         push [edx+12]
         push [edx+8]
         push [edx+4]
         push [edx]
         call function
         movzx eax,al
         mov result,eax }
 return result;
}
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float v;memcpy(&v,(const unsigned char *)p+4*i,4);if(!isfinite(v))return 0;}return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"impact_step_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 unsigned pages[3]={0xe58000,0xe52000,0xe5a000};DWORD protections[3]={0};unsigned char saved[3][4096];unsigned protected=0,error=0,cases=0;
 for(unsigned i=0;i<3;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(saved[i],page,4096);if(!VirtualProtect(page,4096,PAGE_READWRITE,&protections[i])){error=5;break;}++protected;}
 FILE *input=NULL,*output=NULL;unsigned magic=0,total=0;
 if(!error&&(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output))error=6;
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31545350||!read(input,&total,4)||!total||total>1024))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  unsigned h[6],cache[15],ends[4],modes[64],shape_ids[64][2],material_ids[64][2],bodyptrs[16],pool_single[64],pool_two[64],ns=0,nt=0;
  float parameters[11];
  __declspec(align(16)) unsigned char bodies[16][0x430]={0},contacts[64][0x80]={0},shapes[2][0x220]={0},manager[0x100]={0},rows[256][192];
  __declspec(align(16)) float materials[8][16]={0},inertia[16][16],mass[16],workspace[16][8];
  float inertia_before[16][16],mass_before[16];unsigned char manager_before[0x100];
  if(!read(input,h,24)||!h[0]||h[0]>16||!h[1]||h[1]>8||h[2]>64||!h[3]||h[3]>256||h[4]>h[3]||h[5]>32||!read(input,parameters,44)||!finite_words(parameters,11)||parameters[0]<=0||parameters[1]<=0||!isfinite(parameters[0]*parameters[1])||!isfinite(1.f/(parameters[0]*parameters[1]))||!read(input,ends,16)||!read(input,cache,60)){error=8;break;}
  if((cache[0]!=0xffffffff&&cache[0]>=h[1])||(cache[1]!=0xffffffff&&cache[1]>=h[1])||((cache[0]==0xffffffff)!=(cache[1]==0xffffffff))||!finite_words(cache+2,13)){error=8;break;}
  for(unsigned j=0;j<h[0];j++){
   unsigned char *b=bodies[j];float properties[17];
   if(!read(input,b,0x424)||get(b,0x334)!=0x1c0||!finite_words(b+0x110,16)||!finite_words(b+0x1c0,4)||!finite_words(b+0x210,8)||!finite_words(b+0x240,12)||!finite_words(b+0x3f4,1)||!read(input,properties,68)||!finite_words(properties,17)||!read(input,workspace[j],32)||!finite_words(workspace[j],8)){error=8;break;}
   for(unsigned o=0x40c;o<=0x410;o++)if(b[o]>1)error=8;
   put(b,0x334,(unsigned)(uintptr_t)(b+0x1c0));mass[j]=properties[0];memcpy(inertia[j],properties+1,64);bodyptrs[j]=(unsigned)(uintptr_t)b;
  }
  for(unsigned j=0;!error&&j<h[1];j++)if(!read(input,materials[j],52)||!finite_words(materials[j],13))error=8;
  for(unsigned j=0;!error&&j<h[2];j++){
   unsigned char *c=contacts[j];
   if(!read(input,modes+j,4)||modes[j]>1||!read(input,c,128)||get(c,0x1c)>=h[0]||get(c,0x20)>=h[0]||get(c,4)!=get(c,0x1c)||get(c,8)!=get(c,0x20)||get(c,0x30)>=h[1]||get(c,0x34)>=h[1]||get(c,0x2c)>2||get(c,0x28)>2||c[0xe]>1||c[0x70]||c[0x73]>1||!finite_words(c+0x38,2)||!finite_words(c+0x50,8)){error=8;break;}
   float previous;memcpy(&previous,c+0x3c,4);if(previous<0){error=8;break;}
   shape_ids[j][0]=get(c,0x2c);shape_ids[j][1]=get(c,0x28);material_ids[j][0]=get(c,0x30);material_ids[j][1]=get(c,0x34);
   put(c,4,bodyptrs[get(c,0x1c)]);put(c,8,bodyptrs[get(c,0x20)]);
   put(c,0x30,(unsigned)(uintptr_t)materials[material_ids[j][0]]);put(c,0x34,(unsigned)(uintptr_t)materials[material_ids[j][1]]);
   put(c,0x2c,shape_ids[j][0]?(unsigned)(uintptr_t)shapes[shape_ids[j][0]-1]:0);put(c,0x28,shape_ids[j][1]?(unsigned)(uintptr_t)shapes[shape_ids[j][1]-1]:0);
   if(modes[j])pool_two[nt++]=(unsigned)(uintptr_t)c;else pool_single[ns++]=(unsigned)(uintptr_t)c;
  }
  if(error||!read(input,rows,h[3]*192)){error=8;break;}
  shapes[1][0x216]=1;
  for(unsigned j=0;j<2;j++)cache[j]=cache[j]==0xffffffff?0:(unsigned)(uintptr_t)materials[cache[j]];
  memcpy(manager+0x20,cache,60);put(manager,0x70,h[4]);put(manager,0x80,(unsigned)(uintptr_t)workspace);put(manager,0x84,(unsigned)(uintptr_t)inertia);put(manager,0x88,(unsigned)(uintptr_t)mass);put(manager,0x8c,(unsigned)(uintptr_t)rows);put(manager,0x90,(unsigned)(uintptr_t)bodyptrs);put(manager,0x98,(unsigned)(uintptr_t)pool_single);put(manager,0xa0,(unsigned)(uintptr_t)pool_two);put(manager,0xa8,h[0]);put(manager,0xac,ns);put(manager,0xb4,nt);
  put(manager,0xc4,ends[0]);put(manager,0xbc,ends[1]);put(manager,0xc8,ends[2]);put(manager,0xc0,ends[3]);memcpy(manager+0xcc,parameters+5,16);put(manager,0xdc,h[5]);memcpy(image+0xe52f24-0x400000,parameters+9,8);
  memcpy(inertia_before,inertia,sizeof(inertia));memcpy(mass_before,mass,sizeof(mass));memcpy(manager_before,manager,256);
  unsigned success=0;
  __try{success=invoke(image+0xb7acc0-0x400000,manager,(unsigned *)parameters);}
  __except(fault(GetExceptionInformation())){fprintf(stderr,"case=%u\n",i);error=9;}
  if(error)break;
  if(memcmp(inertia_before,inertia,sizeof(inertia))||memcmp(mass_before,mass,sizeof(mass))||memcmp(manager_before,manager,0x20)||memcmp(manager_before+0x5c,manager+0x5c,0x60)||memcmp(manager_before+0xcc,manager+0xcc,0x34)){error=10;break;}
  ends[0]=get(manager,0xc4);ends[1]=get(manager,0xbc);ends[2]=get(manager,0xc8);ends[3]=get(manager,0xc0);memcpy(cache,manager+0x20,60);
  for(unsigned j=0;j<2;j++){if(!cache[j])cache[j]=0xffffffff;else{unsigned found=0;for(unsigned m=0;m<h[1];m++)if(cache[j]==(unsigned)(uintptr_t)materials[m]){cache[j]=m;found=1;break;}if(!found)error=10;}}
  if(success&&h[2])for(unsigned r=ends[1];r<ends[3];r++){
   unsigned offset=get(rows[r],0xa8)-(unsigned)(uintptr_t)rows;
   if(offset%192!=0x98||offset/192>=ends[1]){error=10;break;}put(rows[r],0xa8,offset/192);
  }
  if(error)break;
  fwrite(&success,4,1,output);fwrite(ends,4,4,output);fwrite(cache,4,15,output);fwrite(workspace,32,h[0],output);
  for(unsigned j=0;j<h[0];j++){put(bodies[j],0x334,0x1c0);fwrite(bodies[j],1,0x424,output);}
  for(unsigned j=0;j<h[2];j++){unsigned char *c=contacts[j];put(c,4,get(c,0x1c));put(c,8,get(c,0x20));put(c,0x30,material_ids[j][0]);put(c,0x34,material_ids[j][1]);put(c,0x2c,shape_ids[j][0]);put(c,0x28,shape_ids[j][1]);fwrite(c,1,128,output);}
  fwrite(rows,192,h[3],output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;
 if(input&&fclose(input))error=14;if(output&&fclose(output))error=14;
 free(code);VirtualFree(image,0,MEM_RELEASE);
 printf("impact_step_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
