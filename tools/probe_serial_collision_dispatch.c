/* Whole B78710 serial dispatch, original A064A0 guards; hardware observation. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef struct {unsigned identity;unsigned char bytes[0x424];} Owner;
typedef struct {unsigned identity;unsigned char bytes[0x80],a[0x231],b[0x231];} Contact;
typedef struct {unsigned owner,count,contacts[80];} Trace;
typedef struct {unsigned char *image,*manager;Owner *owners;Contact *contacts;unsigned no,nc,count,consumed,error;Trace trace[32];} Run;
static Run *active;
static void *owner_for(Owner *owners,unsigned n,unsigned id){for(unsigned i=0;i<n;++i)if(owners[i].identity==id)return owners[i].bytes;return NULL;}
static LONG observe(EXCEPTION_POINTERS *e){
 Run *r=active;CONTEXT *c=e->ContextRecord;
 if(e->ExceptionRecord->ExceptionCode!=EXCEPTION_SINGLE_STEP){r->error=9;return EXCEPTION_EXECUTE_HANDLER;}
 if(c->Eip==(DWORD)(r->image+0xb77490-0x400000)){++r->consumed;}
 else if(c->Eip==(DWORD)(r->image+0xa064a0-0x400000)){
  unsigned *stack=(unsigned *)c->Esp,n=stack[3],j;void **contacts=(void **)stack[2];
  if(r->count>=32||n>80){r->error=10;return EXCEPTION_EXECUTE_HANDLER;}
  Trace *t=r->trace+r->count;
  for(j=0;j<r->no&&(void *)stack[1]!=r->owners[j].bytes;++j){}
  if(j==r->no){r->error=10;return EXCEPTION_EXECUTE_HANDLER;}t->owner=r->owners[j].identity;t->count=n;
  for(unsigned i=0;i<n;++i){for(j=0;j<r->nc&&contacts[i]!=r->contacts[j].bytes;++j){}if(j==r->nc){r->error=10;return EXCEPTION_EXECUTE_HANDLER;}t->contacts[i]=r->contacts[j].identity;}
  ++r->count;
 }else{r->error=10;return EXCEPTION_EXECUTE_HANDLER;}
 c->EFlags|=0x10000;c->Dr6=0;return EXCEPTION_CONTINUE_EXECUTION;
}
static DWORD WINAPI worker(void *argument){
 Run *r=argument;void *manager=r->manager,*function=r->image+0xb78710-0x400000;
 __try{__asm {mov ecx,manager
  push 0
  push 0
  mov eax,function
  call eax
 }}__except(observe(GetExceptionInformation())){}
 return r->error;
}
static unsigned execute(Run *r){
 active=r;HANDLE thread=CreateThread(NULL,0,worker,r,CREATE_SUSPENDED,NULL);if(!thread)return 11;
 CONTEXT c={0};c.ContextFlags=CONTEXT_DEBUG_REGISTERS;
 if(!GetThreadContext(thread,&c)){CloseHandle(thread);ExitProcess(11);}
 c.Dr0=(DWORD)(r->image+0xa064a0-0x400000);c.Dr1=(DWORD)(r->image+0xb77490-0x400000);c.Dr6=0;c.Dr7=5;
 if(!SetThreadContext(thread,&c)||ResumeThread(thread)==(DWORD)-1){CloseHandle(thread);ExitProcess(11);}
 if(WaitForSingleObject(thread,30000)!=WAIT_OBJECT_0){CloseHandle(thread);ExitProcess(12);}
 DWORD result=0;if(!GetExitCodeThread(thread,&result))result=11;CloseHandle(thread);active=NULL;return result;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);if(!code)return 4;memcpy(code,image,pe->OptionalHeader.SizeOfImage);
 unsigned char *flag=image+0xf5e1a4-0x400000,saved_flag=*flag;DWORD flag_protection=0;
 if(!VirtualProtect(image+0xf5e000-0x400000,0x1000,PAGE_READWRITE,&flag_protection))return 16;
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0,passes=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x314d4353||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned row=0;row<total;++row){
  unsigned h[5];Owner owners[32]={0};unsigned char manager[0x240]={0};void *pointers[1024];Contact *contacts=NULL;unsigned char *descriptors=NULL;
  if(fread(h,4,5,input)!=5||h[0]>32||h[1]>512||h[2]>512||h[3]>255||!h[4]||h[4]>8){error=7;break;}
  contacts=calloc(h[1]+h[2]+1,sizeof(Contact));descriptors=calloc(h[0]+1,0xa500);if(!contacts||!descriptors){free(contacts);free(descriptors);error=8;break;}
  for(unsigned i=0;i<h[0];++i){unsigned w[2];if(fread(w,4,2,input)!=2||!w[0]||w[1]>1||owner_for(owners,i,w[0])){error=8;break;}owners[i].identity=w[0];*(unsigned *)(owners[i].bytes+0x418)=w[1];*(void **)(owners[i].bytes+0x41c)=image+0xa064a0-0x400000;*(void **)(owners[i].bytes+0x420)=descriptors+i*0xa500;descriptors[i*0xa500+0xa4af]=1;}
  for(unsigned i=0;!error&&i<h[1]+h[2];++i){unsigned w[6];if(fread(w,4,6,input)!=6||!w[0]||w[3]>1||w[4]>1||w[5]>1){error=8;break;}void *a=owner_for(owners,h[0],w[1]),*b=owner_for(owners,h[0],w[2]);if(!a||(i>=h[1]&&(!b||w[3]!=0))){error=8;break;}
   Contact *f=contacts+i;f->identity=w[0];*(void **)(f->bytes+4)=a;*(void **)(f->bytes+8)=b;f->bytes[0xc]=(unsigned char)w[3];*(void **)(f->bytes+0x28)=f->a;*(void **)(f->bytes+0x2c)=f->b;f->a[0x230]=(unsigned char)w[4];f->b[0x230]=(unsigned char)w[5];pointers[i]=f->bytes;
  }
  if(!error){*(void **)(manager+0x98)=pointers;*(void **)(manager+0xa0)=pointers+h[1];*(unsigned *)(manager+0xac)=h[1];*(unsigned *)(manager+0xb4)=h[2];*flag=(unsigned char)h[3];fwrite(h+4,4,1,output);
   for(unsigned pass=0;pass<h[4];++pass){Run r={0};r.image=image;r.manager=manager;r.owners=owners;r.contacts=contacts;r.no=h[0];r.nc=h[1]+h[2];error=execute(&r);if(error)break;
    unsigned state[3]={*flag,r.count,r.consumed};fwrite(state,4,3,output);for(unsigned i=0;i<r.count;++i)fwrite(r.trace+i,4,2+r.trace[i].count,output);++passes;
   }
  }
  free(contacts);free(descriptors);if(error)break;++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=13;
finish:
 *flag=saved_flag;DWORD discarded;if(!VirtualProtect(image+0xf5e000-0x400000,0x1000,flag_protection,&discarded))error=16;
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=14;}
 if(ferror(input)||ferror(output))error=15;int ci=fclose(input),co=fclose(output);if(ci||co)error=15;
 free(code);VirtualFree(image,0,MEM_RELEASE);printf("serial_collision_dispatch_cases=%u passes=%u error=%u code_unchanged=%u no_stubs=1 hardware_observation=1\n",cases,passes,error,error!=14);return (int)error;
}
