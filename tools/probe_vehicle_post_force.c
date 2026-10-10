/* Isolated bounded whole-helper replay, not injection into the running game. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>
#include <io.h>
#include <fcntl.h>
#include "isolated_reference.h"
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
#include "vehicle_post_force_probe.h"
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    unsigned control;if(argc!=4||_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved){VirtualFree(image,0,MEM_RELEASE);return 4;}
    memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
    FILE *input=NULL,*output=NULL;unsigned error=0,count=0,completed=0,created=0;
    if(_wfopen_s(&input,argv[2],L"rb")||!input){error=6;goto finish;}
    unsigned header[2];if(fread(header,8,1,input)!=1||header[0]!=0x31304154||!header[1]||header[1]>64){error=7;goto finish;}
    count=header[1];HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){error=8;goto finish;}
    int fd=_open_osfhandle((intptr_t)handle,_O_BINARY);if(fd<0){CloseHandle(handle);error=8;goto finish;}
    created=1;output=_fdopen(fd,"wb");if(!output){_close(fd);error=8;goto finish;}
    if(fwrite(header,8,1,output)!=1){error=9;goto finish;}
    for(unsigned i=0;i<count;i++){
        unsigned raw_delta;__declspec(align(16)) unsigned char body[0x2600];float delta;
        if(fread(&raw_delta,4,1,input)!=1||fread(body,sizeof(body),1,input)!=1){error=10;break;}
        memcpy(&delta,&raw_delta,4);error=tail_guard(body,delta);if(error)break;
        __try{error=tail_run(image,body,delta);}__except(EXCEPTION_EXECUTE_HANDLER){error=20;}
        if(error)break;
        if(fwrite(&raw_delta,4,1,output)!=1||fwrite(body,sizeof(body),1,output)!=1){error=11;break;}
        ++completed;
    }
    if(!error&&fgetc(input)!=EOF)error=12;
finish:
    if(input)fclose(input);if(output&&fclose(output))error=13;
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    if(!unchanged)error=14;if(error&&created)_wremove(argv[3]);
    free(saved);VirtualFree(image,0,MEM_RELEASE);
    printf("post_force_cases=%u completed=%u code_unchanged=%u error=%u\n",count,completed,unchanged,error);
    return error;
}
