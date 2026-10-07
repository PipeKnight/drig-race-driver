/* Whole original tyre CSV loader/tokenizer with host allocator methods.
   Optional host numeric/handle-generation CRT adapters; no loader stubs. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef unsigned (__fastcall *Load)(void *,void *,const void *,unsigned,void *);
static void *allocations[32];static unsigned sizes[32],allocated,error;
static void *__fastcall allocate(void *self,void *unused,unsigned bytes,unsigned flag){
    (void)self;(void)unused;if(allocated>=32||flag!=1||bytes>8*1024*1024){error=20;return NULL;}
    void *p=calloc(bytes?bytes:1,1);if(!p){error=21;return NULL;}
    allocations[allocated]=p;sizes[allocated++]=bytes;return p;
}
static void __fastcall release(void *self,void *unused,void *p){
    (void)self;(void)unused;for(unsigned i=0;i<allocated;++i)if(allocations[i]==p){free(p);allocations[i]=NULL;return;}error=22;
}
static int valid_pointer(void *p,unsigned bytes){for(unsigned i=0;i<allocated;++i)if(allocations[i]==p&&sizes[i]==bytes)return 1;return 0;}
static double __cdecl numeric(const char *text,char **end){return strtod(text,end);}
static unsigned __cdecl generation(void){return 1;}
static int adapter(unsigned char *entry,void *target,const unsigned char expected[5]){
    if(memcmp(entry,expected,5))return 0;DWORD old,ignored;if(!VirtualProtect(entry,5,PAGE_READWRITE,&old))return 0;
    entry[0]=0xe9;unsigned relative=(unsigned)(uintptr_t)target-(unsigned)(uintptr_t)(entry+5);memcpy(entry+1,&relative,4);
    return VirtualProtect(entry,5,old,&ignored)&&FlushInstructionCache(GetCurrentProcess(),entry,5);
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4&&argc!=5)return 2;int adapted=argc==4;if(argc==5&&wcscmp(argv[4],L"original"))return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    /* Adapter signatures are guarded by the identity-checked mapped originals. */
    if(adapted){
        const unsigned char numeric_prefix[5]={0x6a,0x00,0xff,0x74,0x24};
        unsigned char generation_prefix[5]={0xe8,0xfa,0x8f,0x00,0x00};
        if(!adapter(image+0xbf647d-0x400000,(void *)numeric,numeric_prefix)||
           !adapter(image+0xbf4da7-0x400000,(void *)generation,generation_prefix))return 5;
    }
    DWORD protection,ignored;if(!VirtualProtect(image+0xe85000-0x400000,0x1000,PAGE_READWRITE,&protection))return 6;
    FILE *input=NULL;if(_wfopen_s(&input,argv[2],L"rb")||!input)return 7;long length;
    if(fseek(input,0,SEEK_END)||(length=ftell(input))<=0||length>2*1024*1024||fseek(input,0,SEEK_SET))return 8;
    char *bytes=malloc((size_t)length+1);if(!bytes||fread(bytes,1,(size_t)length,input)!=(size_t)length||fclose(input))return 9;bytes[length]=0;
    unsigned columns=1,cell=0,records=1;for(long i=0;i<length;++i){unsigned char c=(unsigned char)bytes[i];
        if(!c||c>=128||c=='"'){error=10;break;}if(c=='\r'){columns=1;cell=0;if(++records>255){error=10;break;}}
        else if(c==','){cell=0;if(++columns>360){error=10;break;}}else if(c!='\n'&&++cell>63){error=10;break;}
        else if(c=='\n'&&(i==0||bytes[i-1]!='\r')){error=10;break;}}
    if(error)return error;
    void *vtable[5]={0};vtable[2]=(void *)allocate;vtable[4]=(void *)release;void *allocator=vtable;
    unsigned table[17]={0};Load load=(Load)(image+0x9c2dd0-0x400000);unsigned checksum=0;
    __try{checksum=load(table,NULL,bytes,(unsigned)length,&allocator);}
    __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=11;}
    FILE *output=NULL;if(!error&&(_wfopen_s(&output,argv[3],L"wx")||!output))error=12;
    if(!error){unsigned count=table[16],active=((unsigned char *)table)[0x3c];if(count>358||active>1||(!count&&active)||(count&&!valid_pointer((void *)(uintptr_t)table[0],count*4)))error=13;
        else{fprintf(output,"T\t%08x\t%08x\n",active,count);
            for(unsigned i=0;i<count;++i)fprintf(output,"K\t%08x\t%08x\n",i,((unsigned *)(uintptr_t)table[0])[i]);
            for(unsigned row=0;row<13;++row){unsigned *p=(unsigned *)(uintptr_t)table[row+1];if(p&&!valid_pointer(p,count*4)){error=14;break;}
                fprintf(output,"R\t%08x\t%08x",row,p!=NULL);for(unsigned i=0;p&&i<count;++i)fprintf(output,"\t%08x",p[i]);fputc('\n',output);}
        }
    }
    if(output&&(ferror(output)||fclose(output)))error=15;
    for(unsigned i=0;i<allocated;++i)free(allocations[i]);free(bytes);
    if(!VirtualProtect(image+0xe85000-0x400000,0x1000,protection,&ignored))error=16;
    VirtualFree(image,0,MEM_RELEASE);printf("tyre_table_allocations=%u checksum=%08x crt_adapters=%s error=%u\n",allocated,checksum,adapted?"true":"false",error);return error;
}
