/* Whole original 99c990 and 998690, supplied table; no code patches/stubs. */
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
typedef double (__fastcall *Kernel)(void *,void *,float,void *);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5)return 2;
    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
    unsigned char *image=map_reference(argv[1]);if(!image)return 4;
    unsigned char *table=image+0xe8ef80-0x400000;DWORD protection,ignored;
    if(!VirtualProtect(table,0x44,PAGE_READWRITE,&protection))return 5;
    FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 6;
    unsigned count;char tag[4];if(fread(tag,1,4,file)!=4||memcmp(tag,"TYG1",4)||fread(&count,4,1,file)!=1||count>64)return 7;
    unsigned keys[64]={0};float rows[13*64]={0};
    if(fread(keys,4,count,file)!=count||fread(rows,4,count*13,file)!=count*13||fgetc(file)!=EOF||ferror(file)||fclose(file))return 8;
    for(unsigned i=0;i<count*13;++i)if(!isfinite(rows[i]))return 9;
    memset(table,0,0x44);*(unsigned *)table=(unsigned)(uintptr_t)keys;*(unsigned *)(table+0x40)=count;
    for(unsigned row=0;row<13;++row)*(unsigned *)(table+4+row*4)=(unsigned)(uintptr_t)(rows+row*count);
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[3],L"r")||!input||_wfopen_s(&output,argv[4],L"wx")||!output)return 10;
    Kernel kernel=(Kernel)(image+0x99c990-0x400000);char line[1024],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++cases>4096||!strchr(line,'\n')){error=11;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=12;break;}memcpy(name,start,length);name[length]=0;
        unsigned words[8];
        for(unsigned i=0;i<8;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;unsigned long value=strtoul(cursor,&stop,16);if(stop-cursor!=8){error=13;break;}words[i]=(unsigned)value;cursor=stop;}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||words[0]>1||words[2]>1||words[3]>=13){error=14;break;}
        if(words[0]&&!words[2]&&words[1]>=count){error=15;break;}
        float values[4];memcpy(values,words+4,16);for(unsigned i=0;i<4;++i)if(!isfinite(values[i]))error=16;if(error||values[1]<0.f){error=16;break;}
        unsigned char material[0xb0]={0};memcpy(material,values,12);*(unsigned *)(material+0xa8)=words[1];material[0xac]=(unsigned char)words[2];table[0x3c]=(unsigned char)words[0];
        unsigned selector[2]={0,words[3]};float result=0;
        __try{result=(float)kernel(material,NULL,values[3],selector);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=17;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\t%08x\n",name,bits(result),*(unsigned *)(material+0xa8),(unsigned)material[0xac]);
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=18;
    if(!VirtualProtect(table,0x44,protection,&ignored))error=19;
    VirtualFree(image,0,MEM_RELEASE);printf("tyre_grip_cases=%u error=%u\n",cases,error);return error;
}
