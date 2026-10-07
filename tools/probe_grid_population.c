/* Whole original name parser, supplied-exponent allocator/initializer and insertion.
   Only allocator virtual methods are supplied by the host; no code patches. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef int (__cdecl *Parse)(const char *,unsigned *,unsigned *);
typedef void (__cdecl *Initialize)(unsigned *,void *);
typedef void (__cdecl *Insert)(unsigned,unsigned,unsigned,unsigned,unsigned);
static void *allocated=NULL;
static void *__fastcall allocate(void *self,void *unused,unsigned bytes,unsigned flag){
    (void)self;(void)unused;if(allocated||flag!=1||bytes>4*1024*1024)return NULL;
    allocated=malloc(bytes);return allocated;
}
static void __fastcall release(void *self,void *unused,void *p){(void)self;(void)unused;(void)p;}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    DWORD old,ignored;if(!VirtualProtect(image+0xe81000-0x400000,0x5000,PAGE_READWRITE,&old))return 4;
    Parse parse=(Parse)(image+0x5b4d40-0x400000);Initialize initialize=(Initialize)(image+0x96b660-0x400000);Insert insert=(Insert)(image+0x958470-0x400000);
    FILE *input=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input)return 5;
    unsigned paths[512][3],count=0,exponents[1024]={0};char line[128];
    while(fgets(line,sizeof(line),input)){
        size_t length=strlen(line);while(length&&(line[length-1]=='\n'||line[length-1]=='\r'))line[--length]=0;
        if(count>=512||length<11||length>63||strncmp(line,"qt_",3)||strcmp(line+length-6,".vcqtc"))return 6;
        unsigned depth=0;char *p=line+2;while(*p=='_'){if((p[1]!='0'&&p[1]!='1')||(p[2]!='0'&&p[2]!='1')||++depth>11)return 7;p+=3;}if(strcmp(p,".vcqtc")||!depth)return 8;
        unsigned x=0,z=0;int actual=parse(line,&x,&z);if(actual!=(int)depth||x>=65536||z>=65536)return 9;
        paths[count][0]=x;paths[count][1]=z;paths[count][2]=depth;
        unsigned tile=(z/2048)*32+x/2048,exponent=depth>5?depth-5:0;if(exponents[tile]<exponent)exponents[tile]=exponent;count++;
    }
    if(ferror(input)||fclose(input)||!count)return 10;
    unsigned cells=0;for(unsigned i=0;i<1024;++i)cells+=1u<<(2*exponents[i]);if(cells>1024*1024)return 11;
    void *vtable[5]={0};vtable[2]=(void *)allocate;vtable[4]=(void *)release;void *allocator=vtable;
    *(unsigned *)(image+0xe85b14-0x400000)=0;*(unsigned *)(image+0xe85b1c-0x400000)=0;
    __try{initialize(exponents,&allocator);if(!allocated)return 12;for(unsigned i=0;i<count;++i){unsigned size=65536u>>paths[i][2];insert(i+1,paths[i][0],paths[i][1],paths[i][0]+size-1,paths[i][1]+size-1);}}
    __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());return 13;}
    if(*(unsigned *)(image+0xe85b1c-0x400000)!=count||*(unsigned *)(image+0xe85b14-0x400000)!=(unsigned)(uintptr_t)allocated)return 14;
    for(unsigned i=0;i<count;++i)if(*(unsigned *)(image+0xe83af8+i*4-0x400000)!=i+1)return 15;
    FILE *output=NULL;if(_wfopen_s(&output,argv[3],L"wbx")||!output)return 16;
    int ok=fwrite("GBL1",1,4,output)==4&&fwrite(&count,4,1,output)==1&&fwrite(paths,12,count,output)==count;
    unsigned *widths=(unsigned *)(image+0xe81af8-0x400000),*actual_exponents=(unsigned *)(image+0xe82af8-0x400000),*pointers=(unsigned *)(image+0xe84af8-0x400000);
    for(unsigned i=0;i<1024;++i){if(actual_exponents[i]!=exponents[i]||widths[i]!=(1u<<exponents[i]))return 17;unsigned n=widths[i]*widths[i];ok=ok&&fwrite(actual_exponents+i,4,1,output)==1&&fwrite(widths+i,4,1,output)==1&&fwrite((void *)(uintptr_t)pointers[i],4,n,output)==n;}
    if(fclose(output))ok=0;free(allocated);if(!VirtualProtect(image+0xe81000-0x400000,0x5000,old,&ignored))ok=0;VirtualFree(image,0,MEM_RELEASE);
    printf("paths=%u cells=%u exact_tables=%d\n",count,cells,ok);return ok?0:18;
}
