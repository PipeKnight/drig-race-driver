/* Whole original initial contact builders and original dependencies; CRW1. */
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
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static int read(FILE *f,void *p,size_t n){return fread(p,1,n,f)==n;}
static int finite_words(const void *p,unsigned n){for(unsigned i=0;i<n;i++){float v;memcpy(&v,(const unsigned char *)p+4*i,4);if(!isfinite(v))return 0;}return 1;}
static int fault(EXCEPTION_POINTERS *p){fprintf(stderr,"contact_rows_fault=%08lx eip=%08lx address=%08lx\n",p->ExceptionRecord->ExceptionCode,p->ContextRecord->Eip,p->ExceptionRecord->NumberParameters>1?(unsigned long)p->ExceptionRecord->ExceptionInformation[1]:0);return EXCEPTION_EXECUTE_HANDLER;}
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
 if(!error&&(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||!read(input,&magic,4)||magic!=0x31575243||!read(input,&total,4)||!total||total>1024))error=7;
 if(error)goto finish;
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned i=0;i<total;i++){
  unsigned h[3],cache[15];float parameters[6];
  __declspec(align(16)) unsigned char bodies[16][0x430]={0},shapes[2][0x220]={0},contacts[64][0x80]={0},context[0x90]={0};
  unsigned char body_wire[16][56],contact_wire[64][80],body_before[16][0x430],contact_before[64][128],context_before[0x90];
  __declspec(align(16)) float materials[8][16]={0};
  unsigned char *rows[4]={0};unsigned capacities[4]={0};
  if(!read(input,h,12)||!h[0]||h[0]>16||!h[1]||h[1]>8||h[2]>64||!read(input,parameters,24)||!finite_words(parameters,6)||parameters[0]<=0||!read(input,cache,60)){error=8;break;}
  if((cache[0]!=0xffffffff&&cache[0]>=h[1])||(cache[1]!=0xffffffff&&cache[1]>=h[1])||((cache[0]==0xffffffff)!=(cache[1]==0xffffffff))||!finite_words(cache+2,13)){error=8;break;}
  for(unsigned j=0;j<h[0];j++){
   if(!read(input,body_wire[j],56)||!finite_words(body_wire[j],12)||get(body_wire[j],48)>1||get(body_wire[j],52)>1){error=8;break;}
   memcpy(bodies[j]+0x1c0,body_wire[j],16);memcpy(bodies[j]+0x210,body_wire[j]+16,32);
   bodies[j][0x410]=(unsigned char)get(body_wire[j],48);bodies[j][0x40f]=(unsigned char)get(body_wire[j],52);
   put(bodies[j],0x334,(unsigned)(uintptr_t)(bodies[j]+0x1c0));
  }
  for(unsigned j=0;!error&&j<h[1];j++)if(!read(input,materials[j],52)||!finite_words(materials[j],13))error=8;
  for(unsigned j=0;!error&&j<h[2];j++){
   unsigned char *w=contact_wire[j],*c=contacts[j];
   if(!read(input,w,80)||get(w,0)>1||get(w,4)>=h[0]||get(w,8)>=h[0]||get(w,12)>=h[1]||get(w,16)>=h[1]||get(w,20)>2||get(w,24)>2||get(w,28)>1||get(w,32)>1||get(w,36)>1||!finite_words(w+40,10)){error=8;break;}
   put(c,4,(unsigned)(uintptr_t)bodies[get(w,4)]);put(c,8,(unsigned)(uintptr_t)bodies[get(w,8)]);
   put(c,0x1c,get(w,4));put(c,0x20,get(w,8));
   put(c,0x30,(unsigned)(uintptr_t)materials[get(w,12)]);put(c,0x34,(unsigned)(uintptr_t)materials[get(w,16)]);
   put(c,0x2c,get(w,20)?(unsigned)(uintptr_t)shapes[get(w,20)-1]:0);
   put(c,0x28,get(w,24)?(unsigned)(uintptr_t)shapes[get(w,24)-1]:0);
   c[0x73]=(unsigned char)get(w,28);c[0x70]=(unsigned char)get(w,32);c[0xe]=(unsigned char)get(w,36);
   memcpy(c+0x38,w+40,8);memcpy(c+0x50,w+48,32);
  }
  shapes[1][0x216]=1;
  for(unsigned p=0;!error&&p<4;p++){
   capacities[p]=(h[2]?h[2]:1)*(p%2?3:1);rows[p]=_aligned_malloc(capacities[p]*192,16);
   if(!rows[p]||!read(input,rows[p],capacities[p]*192))error=8;
  }
  if(error)goto case_done;
  memcpy(body_before,bodies,sizeof(bodies));memcpy(contact_before,contacts,sizeof(contacts));
  for(unsigned j=0;j<2;j++)cache[j]=cache[j]==0xffffffff?0:(unsigned)(uintptr_t)materials[cache[j]];
  memcpy(context+4,parameters,12);memcpy(context+0x14,parameters+3,4);
  memcpy(image+0xe52f24-0x400000,parameters+4,8);put(context,0x58,(unsigned)(uintptr_t)cache);
  put(context,0x38,(unsigned)(uintptr_t)rows[0]);put(context,0x3c,(unsigned)(uintptr_t)rows[1]);
  put(context,0x40,(unsigned)(uintptr_t)rows[2]);put(context,0x44,(unsigned)(uintptr_t)rows[3]);
  memcpy(context_before,context,sizeof(context));
  for(unsigned j=0;j<h[2];j++){
   __try{invoke(image+(get(contact_wire[j],0)?0xb0d1b0:0xb0e040)-0x400000,contacts[j],context);}
   __except(fault(GetExceptionInformation())){fprintf(stderr,"case=%u contact=%u\n",i,j);error=9;}
   if(error)break;
  }
  if(error)goto case_done;
  if(memcmp(body_before,bodies,sizeof(bodies))||memcmp(contact_before,contacts,sizeof(contacts))||
     memcmp(context_before,context,0x24)||memcmp(context_before+0x58,context+0x58,8)||
     memcmp(context_before+0x88,context+0x88,8)){error=10;goto case_done;}
  unsigned counts[4]={get(context,0x28),get(context,0x30),get(context,0x24),get(context,0x2c)};
  unsigned summary[13];memcpy(summary,counts,16);summary[4]=get(context,0x34);
  unsigned cursor_offsets[4]={0x38,0x3c,0x40,0x44},head_offsets[4]={0x48,0x4c,0x50,0x54};
  for(unsigned p=0;p<4;p++){
   unsigned cursor=get(context,cursor_offsets[p])-(unsigned)(uintptr_t)rows[p],head=get(context,head_offsets[p]);
   if(cursor%192||cursor/192>capacities[p]){error=10;break;}summary[5+p]=cursor;
   summary[9+p]=head?head-(unsigned)(uintptr_t)rows[p]:0xffffffff;
   if(head&&(summary[9+p]%192||summary[9+p]/192>=cursor/192)){error=10;break;}
   for(unsigned r=0;r<cursor/192;r++){
    unsigned char *row=rows[p]+r*192;
    if(!finite_words(row,p>=2?16:8)||!finite_words(row+(p%2?0x88:0x80),p%2?2:4)){error=10;break;}
    if(p%2){unsigned ptr=get(row,0xa8),normal=p-1,offset=ptr-(unsigned)(uintptr_t)rows[normal];
     if(offset%192!=0x98||offset/192>=capacities[normal]){error=10;break;}put(row,0xa8,((normal+1)<<24)|offset);}
   }
  }
  if(error)goto case_done;
  for(unsigned j=0;j<2;j++){
   if(!cache[j])cache[j]=0xffffffff;
   else{unsigned found=0;for(unsigned m=0;m<h[1];m++)if(cache[j]==(unsigned)(uintptr_t)materials[m]){cache[j]=m;found=1;break;}if(!found)error=10;}
  }
  if(error)goto case_done;
  fwrite(summary,4,13,output);fwrite(context+0x60,1,40,output);fwrite(cache,4,15,output);
  for(unsigned p=0;p<4;p++)fwrite(rows[p],1,capacities[p]*192,output);++cases;
case_done:
  for(unsigned p=0;p<4;p++)if(rows[p])_aligned_free(rows[p]);
  if(error)break;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<protected;i++){unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved[i],4096);DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=12;}
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=13;}
 if(input&&ferror(input))error=14;if(output&&ferror(output))error=14;
 if(input&&fclose(input))error=14;if(output&&fclose(output))error=14;
 free(code);VirtualFree(image,0,MEM_RELEASE);
 printf("contact_rows_cases=%u error=%u code_unchanged=%u no_imports=1\n",cases,error,error!=13);return (int)error;
}
