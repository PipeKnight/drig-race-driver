/* Whole original candidate collector and coordinate/subdivision helpers.
   Authored bounds/grid fixture; IDs are tokens, not dereferenced objects. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <io.h>
#include <fcntl.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef void (__cdecl *Coordinates)(float,float,int *,int *);
typedef int (__cdecl *Collect)(unsigned *,int,float,float,float,float);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    DWORD old,ignored;if(!VirtualProtect(image+0xe81000-0x400000,0x5000,PAGE_READWRITE,&old))return 4;
    unsigned *widths=(unsigned *)(image+0xe81af8-0x400000),*exponents=(unsigned *)(image+0xe82af8-0x400000),*pointers=(unsigned *)(image+0xe84af8-0x400000);
    unsigned wrapper[3]={0};unsigned char object[0x248]={0};float bounds[6]={0};
    wrapper[2]=(unsigned)(uintptr_t)object;*(unsigned *)(object+0x244)=(unsigned)(uintptr_t)bounds;
    *(unsigned *)(image+0xe83af8-0x400000)=(unsigned)(uintptr_t)wrapper;
    unsigned cells[1024][64];for(unsigned i=0;i<1024;++i)pointers[i]=(unsigned)(uintptr_t)cells[i];
    FILE *input=NULL;if(_wfopen_s(&input,argv[2],L"r")||!input)return 5;
    HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)return 6;
    int descriptor=_open_osfhandle((intptr_t)h,_O_TEXT);if(descriptor<0){CloseHandle(h);return 7;}
    FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);return 8;}
    Coordinates coordinates=(Coordinates)(image+0x96b390-0x400000);Collect collect=(Collect)(image+0x96b440-0x400000);
    char line[2048],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++count>256||!strchr(line,'\n')){error=9;break;}
        char *cursor=line,*begin=cursor;while(*cursor&&*cursor!='\t')++cursor;
        size_t n=(size_t)(cursor-begin);if(!n||n>=sizeof(name)){error=10;break;}memcpy(name,begin,n);name[n]=0;
        unsigned words[12];for(unsigned i=0;i<12;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *end;words[i]=strtoul(cursor,&end,16);if(end-cursor!=8){error=11;break;}cursor=end;}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=12;break;}
        float values[8];memcpy(values,words,32);for(unsigned i=0;i<8;++i)if(!isfinite(values[i]))error=13;
        unsigned capacity=words[8],seed=words[9],exponent=words[10],active=words[11];
        if(error||capacity>25||seed>1000||exponent>3||active>1||values[2]<=values[0]||values[3]<=values[1]||values[4]>values[6]||values[5]>values[7]){error=14;break;}
        bounds[0]=values[0];bounds[2]=values[1];bounds[3]=values[2];bounds[5]=values[3];
        for(unsigned i=4;i<8;++i){unsigned a=(i-4)%2;float q=((values[i]-values[a])/(values[a+2]-values[a]))*65536.0f;if(!isfinite(q)||q<-2147483648.0f||q>=2147483648.0f)error=15;}
        if(error)break;
        unsigned width=1u<<exponent;
        for(unsigned tile=0;tile<1024;++tile){widths[tile]=width;exponents[tile]=exponent;
            for(unsigned c=0;c<width*width;++c){unsigned key=(tile%32)*13+(tile/32)*7+(c%width)*5+(c/width)*11+seed;cells[tile][c]=key%5?key%(seed%31+1)+1:0;}
        }
        *(unsigned *)(image+0xe85b1c-0x400000)=active;
        int a[2],b[2],total=0;unsigned ids[25]={0};
        __try{coordinates(values[4],values[5],a,a+1);coordinates(values[6],values[7],b,b+1);total=collect(ids,(int)capacity,values[4],values[5],values[6],values[7]);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=16;break;}
        if(total<0||total>25){error=17;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\t%08x\t%08x\t%08x",name,a[0],a[1],b[0],b[1],total);
        for(int i=0;i<total;++i)fprintf(output,"\t%08x",ids[i]);fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=18;if(fclose(input)||fclose(output))error=19;
    if(!VirtualProtect(image+0xe81000-0x400000,0x5000,old,&ignored))error=20;
    VirtualFree(image,0,MEM_RELEASE);printf("candidate_grid_cases=%u error=%u\n",count,error);return error;
}
