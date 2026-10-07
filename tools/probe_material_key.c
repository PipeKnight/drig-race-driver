/* Whole original raw packed key lookup; supplied key table, no stubs/patches. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"
typedef unsigned *(__fastcall *Lookup)(void *,void *,unsigned);
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;Lookup lookup=(Lookup)(image+0xb3b0c0-0x400000);
    FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
    unsigned count=0;char tag[4];if(fread(tag,1,4,file)!=4||memcmp(tag,"KEY1",4)||fread(&count,4,1,file)!=1||count>4096)return 5;
    unsigned *keys=count?malloc(count*4):NULL;if(count&&(!keys||fread(keys,4,count,file)!=count))return 6;if(fgetc(file)!=EOF||ferror(file)||fclose(file))return 7;
    unsigned table[5]={0,(unsigned)(uintptr_t)keys,count,4,0};
    FILE *input=NULL,*output=NULL;if(_wfopen_s(&input,argv[3],L"r")||!input||_wfopen_s(&output,argv[4],L"wx")||!output)return 8;
    char line[256],name[64];unsigned cases=0,error=0;
    while(fgets(line,sizeof(line),input)){unsigned code,active;char extra;int parsed=sscanf_s(line,"%63s %8x %8x %c",name,(unsigned)sizeof(name),&code,&active,&extra,1);if(++cases>4096||parsed!=3||active>1){error=9;break;}table[4]=active;
        unsigned *result=NULL;__try{result=lookup(table,NULL,code);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"fault=%08lx\n",GetExceptionCode());error=10;break;}
        if(result&&(result<keys||result>=keys+count)){error=11;break;}
        fprintf(output,"P\t%s\t%08x\t%08x\n",name,result!=NULL,result?(unsigned)(result-keys):0);
    }
    if(ferror(input)||ferror(output)||fclose(input)||fclose(output))error=12;free(keys);VirtualFree(image,0,MEM_RELEASE);printf("material_key_cases=%u error=%u\n",cases,error);return error;
}
