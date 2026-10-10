/* Joined original sparse creation/substitution/selected/remaining solve.
   Owned bounded buffers, no helper stubs, code patches or live injection. */
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
#include "isolated_reference.h"

typedef void (__fastcall *Add)(void *,void *,unsigned,const unsigned *,const float *,float);
typedef void (__fastcall *Fix)(void *,void *,unsigned,float);
typedef unsigned char (__fastcall *Select)(void *,void *,const unsigned *,const unsigned *,unsigned,unsigned);
typedef unsigned char (__fastcall *Remaining)(void *,void *);

static int finite_word(unsigned word) { float v;memcpy(&v,&word,4);return isfinite(v)&&bits(v)==word; }
static int closed(const unsigned *state,const unsigned *rows,const unsigned *vars,unsigned n) {
    if(!n||n>40)return 0;
    for(unsigned i=0;i<n;i++) {
        if(rows[i]>=state[3]||vars[i]>=state[2])return 0;
        for(unsigned j=0;j<i;j++)if(rows[j]==rows[i]||vars[j]==vars[i])return 0;
    }
    for(unsigned i=0;i<n;i++){
        const unsigned *row=state+84+rows[i]*19;
        for(unsigned j=0;j<row[0];j++){
            unsigned found=0;for(unsigned k=0;k<n;k++)if(row[9+j]==vars[k])found=1;
            if(!found)return 0;
        }
    }
    return 1;
}

int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved=malloc(pe->OptionalHeader.SizeOfImage);if(!saved)return 4;
    memcpy(saved,image,pe->OptionalHeader.SizeOfImage);
    FILE *input=NULL,*output=NULL;unsigned error=0,cases=0,operations=0;
    if(_wfopen_s(&input,argv[2],L"r")||!input){error=5;goto finish;}
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){error=6;goto finish;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);error=6;goto finish;}
    output=_fdopen(descriptor,"w");if(!output){_close(descriptor);error=6;goto finish;}
    Add add=(Add)(image+0x992320-0x400000);
    Fix fix=(Fix)(image+0x9930a0-0x400000);
    Select select=(Select)(image+0x9925e0-0x400000);
    Remaining remaining=(Remaining)(image+0x992fd0-0x400000);
    char *line=malloc(73728);if(!line){error=7;goto finish;}
    while(fgets(line,73728,input)){
        if(++cases>1024||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor,name[64];
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        for(unsigned i=0;i<length;i++)if(!((name[i]>='a'&&name[i]<='z')||(name[i]>='A'&&name[i]<='Z')||(name[i]>='0'&&name[i]<='9')||name[i]=='_'))error=9;
        unsigned words[8192],size=0;
        while(!error){
            while(*cursor==' '||*cursor=='\t'||*cursor=='\r'||*cursor=='\n')++cursor;
            if(!*cursor)break;
            if(size==8192){error=10;break;}
            char *stop;unsigned v=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            words[size++]=v;cursor=stop;
        }
        if(error||size<2||words[0]>40||!words[1]||words[1]>128){error=11;break;}
        __declspec(align(16)) unsigned state[884]={0};state[2]=words[0];
        unsigned at=2;
        for(unsigned operation=0;operation<words[1];operation++){
            if(operations==20000){error=12;break;}
            if(at>=size){error=12;break;}
            unsigned code=words[at++],n=0,rows[40],vars[40],success=1;
            float coefficients[8],value;
            if(code==0){
                if(at>=size||(n=words[at++])>8||state[3]>=40||size-at<2*n+1){error=13;break;}
                for(unsigned i=0;i<n;i++){
                    vars[i]=words[at++];unsigned word=words[at++];memcpy(coefficients+i,&word,4);
                    if(vars[i]>=state[2]||!finite_word(word))error=13;
                }
                if(!finite_word(words[at]))error=13;
                memcpy(&value,words+at++,4);
                if(error)break;
                __try {add(state,NULL,n,vars,coefficients,value);}
                __except(EXCEPTION_EXECUTE_HANDLER){error=20;}
            }else if(code==1){
                if(size-at<2||words[at]>=state[2]||!finite_word(words[at+1])){error=14;break;}
                unsigned id=words[at++];memcpy(&value,words+at++,4);
                __try {fix(state,NULL,id,value);}
                __except(EXCEPTION_EXECUTE_HANDLER){error=20;}
            }else if(code==2){
                if(at>=size||(n=words[at++])==0||n>40||size-at<2*n){error=15;break;}
                memcpy(rows,words+at,n*4);at+=n;memcpy(vars,words+at,n*4);at+=n;
                if(!closed(state,rows,vars,n)){error=15;break;}
                __try {success=select(state,NULL,rows,vars,n,1)!=0;}
                __except(EXCEPTION_EXECUTE_HANDLER){error=20;}
            }else if(code==3){
                unsigned row_count=0;
                for(unsigned i=0;i<state[2];i++)if(!(state[5+2*i]&0xffff))vars[n++]=i;
                for(unsigned i=0;i<state[3];i++)if(!state[84+19*i+18]&&state[84+19*i])rows[row_count++]=i;
                if(n&&(row_count<n||!closed(state,rows,vars,n))){error=16;break;}
                __try {success=remaining(state,NULL)!=0;}
                __except(EXCEPTION_EXECUTE_HANDLER){error=20;}
            }else{error=17;break;}
            if(error){fprintf(output,"F\t%s\t%u\t%u\n",name,operation,error);break;}
            fprintf(output,"P\t%s\t%u\t%08x",name,operation,success);
            for(unsigned i=0;i<884;i++)fprintf(output,"\t%08x",state[i]);
            fputc('\n',output);++operations;
        }
        if(error||at!=size){if(!error)error=18;break;}
    }
    free(line);
    if(ferror(input)||ferror(output))error=19;
finish:
    if(input)fclose(input);if(output&&fclose(output))error=21;
    unsigned unchanged=1;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++){
        if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&
           memcmp(image+sections[i].VirtualAddress,saved+sections[i].VirtualAddress,sections[i].SizeOfRawData))unchanged=0;
    }
    if(!unchanged)error=22;
    free(saved);VirtualFree(image,0,MEM_RELEASE);
    printf("force_equation_cases=%u operations=%u code_unchanged=%u error=%u\n",cases,operations,unchanged,error);
    return error;
}
