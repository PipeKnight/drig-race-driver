/* Whole range/default-slider builders, supplied decoded suspension subset.
   x87 53-bit nearest explicitly selected; no stubs or live injection. */
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
#include <float.h>
#include "isolated_reference.h"

typedef void (__cdecl *Initialize)(void *,void *);
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);
    if(!image)return 3;
    const unsigned pages[]={0xe58000,0xe8e000};DWORD protection[2];
    for(unsigned i=0;i<2;++i)if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protection+i))return 16;
    unsigned control;
    if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 17;
    printf("x87_control=%08x\n",control);
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[2],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    Initialize initialize=(Initialize)(image+0x998e60-0x400000);
    char line[4096],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[36];
        for(unsigned i=0;i<36;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||(values[0]!=0.f&&values[0]!=1.f)){error=12;break;}
        __declspec(align(16)) unsigned char record[0x660]={0};
        __declspec(align(16)) unsigned output_sliders[33]={0};
        const unsigned fields[]={0x3e0,0x40c,0x410,0x440,0x444,0x438,0x43c,0x448,0x44c,0x450,0x454,0x414,0x418};
        const unsigned indices[]={4,5,6,7,8,9,10,11,12,28,29};
        for(unsigned j=0;j<13;++j)memcpy(record+fields[j],values+1+j,4);
        if(values[0]==1.f)for(unsigned j=0;j<11;++j){
            memcpy(record+0xb8+indices[j]*4,values+14+j*2,4);
            memcpy(record+0x40+indices[j]*4,values+15+j*2,4);
        }
        Initialize defaults=(Initialize)(image+0x9c3c30-0x400000);
        __try {
            if(values[0]==0.f)initialize(record,record+0x40);
            defaults(record,output_sliders);
        }
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned j=0;j<11;++j)fprintf(output,"\t%08x\t%08x\t%08x",
            *(unsigned *)(record+0xb8+indices[j]*4),*(unsigned *)(record+0x40+indices[j]*4),
            output_sliders[3+indices[j]]);
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
    for(unsigned i=0;i<2;++i){DWORD ignored;if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protection[i],&ignored))error=18;}
    VirtualFree(image,0,MEM_RELEASE);
    printf("range_cases=%u error=%u\n",count,error);
    return error;
}
