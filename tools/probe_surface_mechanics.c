/* Original mechanics loader with original binary-XML views and defaults.
   Numeric CRT conversion adapter uses host strtod; original thread/locale CRT
   initialization is not executed. Thus full original text parsing is NOT claimed. */
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

typedef void (__cdecl *Load)(void *,void *);
static double __cdecl numeric_adapter(const char *text){return strtod(text,NULL);}
static int adapt_numeric(unsigned char *image){
    unsigned char *entry=image+0xbf3b60-0x400000;DWORD old,ignored;
    if(entry[0]!=0x6a||entry[1]!=0)return 0;
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&old))return 0;
    entry[0]=0xe9;uint32_t relative=(uint32_t)(uintptr_t)&numeric_adapter-(uint32_t)(uintptr_t)(entry+5);
    memcpy(entry+1,&relative,4);
    return VirtualProtect(entry,5,old,&ignored)&&FlushInstructionCache(GetCurrentProcess(),entry,5);
}
static unsigned word(const void *p){unsigned v;memcpy(&v,p,4);return v;}
static const char *text(unsigned *doc,unsigned id){
    if(id>=doc[6])return NULL;
    return (const char *)(uintptr_t)(doc[7]+word((unsigned char *)(uintptr_t)doc[8]+id*4));
}
static unsigned char *read_xml(const wchar_t *path,unsigned *doc){
    FILE *file=NULL;if(_wfopen_s(&file,path,L"rb")||!file)return NULL;
    long size;if(fseek(file,0,SEEK_END)||(size=ftell(file))<48||size>2*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return NULL;}
    unsigned char *raw=malloc((size_t)size);if(!raw){fclose(file);return NULL;}
    int success=fread(raw,1,(size_t)size,file)==(size_t)size;fclose(file);if(!success)goto fail;
    if(word(raw)!=0x7252221a||word(raw+4)!=(unsigned)size-8||word(raw+8)!=0x72522217)goto fail;
    const unsigned tags[]={0x7252221d,0x7252221e,0x7252221b,0x7252221c};unsigned cursor=16,lengths[4];unsigned char *parts[4];
    for(unsigned i=0;i<4;++i){
        if(!inside(cursor,8,(size_t)size)||word(raw+cursor)!=tags[i])goto fail;
        unsigned length=word(raw+cursor+4);cursor+=8;
        if(!inside(cursor,length,(size_t)size))goto fail;
        lengths[i]=length;parts[i]=raw+cursor;cursor+=length;
    }
    if(cursor!=(unsigned)size||lengths[1]%4||lengths[2]%24||lengths[3]%8||word(raw+12)!=lengths[0]+lengths[1]+16)goto fail;
    doc[2]=lengths[2]/24;doc[3]=(unsigned)(uintptr_t)parts[2];doc[4]=lengths[3]/8;doc[5]=(unsigned)(uintptr_t)parts[3];
    doc[6]=lengths[1]/4;doc[7]=(unsigned)(uintptr_t)parts[0];doc[8]=(unsigned)(uintptr_t)parts[1];
    if(doc[2]>10000||doc[4]>50000||doc[6]>10000)goto fail;
    for(unsigned i=0;i<doc[6];++i){unsigned offset=word(parts[1]+i*4);if(offset>=lengths[0]||!memchr(parts[0]+offset,0,lengths[0]-offset))goto fail;}
    for(unsigned i=0;i<doc[2];++i){unsigned *e=(unsigned *)(parts[2]+i*24);
        if(e[0]>=doc[6]||e[1]>=doc[6]||!inside(e[3],e[2],doc[4])||!inside(e[5],e[4],doc[2]))goto fail;
    }
    for(unsigned i=0;i<doc[4]*2;++i)if(word(parts[3]+i*4)>=doc[6])goto fail;
    return raw;
fail:
    free(raw);return NULL;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4&&argc!=5)return 2;
    int force=argc==5&&!wcscmp(argv[4],L"force"),adapted=argc==4||force;if(argc==5&&!force&&wcscmp(argv[4],L"original"))return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;if(adapted&&!adapt_numeric(image))return 4;
    unsigned doc[9]={0};unsigned char *xml=read_xml(argv[2],doc);if(!xml)return 5;
    unsigned *root=(unsigned *)(uintptr_t)doc[3];if(strcmp(text(doc,root[0]),"MATERIALS")){free(xml);return 6;}
    HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE){free(xml);return 7;}
    int descriptor=_open_osfhandle((intptr_t)h,_O_TEXT);if(descriptor<0){CloseHandle(h);free(xml);return 8;}
    FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);free(xml);return 9;}
    Load load=(Load)(image+0x9b90f0-0x400000);unsigned count=0,valid=0,error=0;
    for(unsigned i=0;i<root[4];++i){
        unsigned *e=(unsigned *)((unsigned char *)(uintptr_t)doc[3]+(root[5]+i)*24);
        if(strcmp(text(doc,e[0]),"MATERIAL"))continue;
        const char *name=NULL;for(unsigned a=0;a<e[2];++a){unsigned *p=(unsigned *)((unsigned char *)(uintptr_t)doc[5]+(e[3]+a)*8);if(strcmp(text(doc,p[0]),"name")==0)name=text(doc,p[1]);}
        if(!name||strlen(name)!=4){error=10;break;}
        unsigned has=0;for(unsigned c=0;c<e[4];++c){unsigned *p=(unsigned *)((unsigned char *)(uintptr_t)doc[3]+(e[5]+c)*24);if(strcmp(text(doc,p[0]),"MECHANICS")==0)++has;}
        if(has>1){error=11;break;}
        unsigned view[11]={0};memcpy(view+2,root,24);view[1]=i;view[8]=(unsigned)(uintptr_t)doc;
        __declspec(align(16)) unsigned char material[0xb0];memset(material,0xcd,sizeof(material));
        __try{load(material,view);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%u\t%08lx\n",i,GetExceptionCode());error=12;break;}
        fprintf(output,"M\t%08x\t%08x",word(name),has);
        const unsigned offsets[]={0x90,0x94,0x98,0x9c,0xa0,0xa4,8,0x64,0xa8};
        for(unsigned a=0;a<9;++a){unsigned v=has?(a==0?material[offsets[a]]:word(material+offsets[a])):0;fprintf(output,"\t%08x",v);}
        if(force){const unsigned extra[]={0,4,0xc,0x10,0x14,0x18,0x1c,0x20,0x68,0x6c,0x71,0x72,0x75};
            for(unsigned a=0;a<13;++a){unsigned v=has?(a>=10?material[extra[a]]:word(material+extra[a])):0;fprintf(output,"\t%08x",v);}}
        fputc('\t',output);
        if(has){unsigned n=0;while(n<60&&material[0x24+n]){fprintf(output,"%02x",material[0x24+n]);++n;}if(n==60){error=13;break;}}
        fputc('\n',output);++count;valid+=has;
    }
    if(ferror(output))error=14;if(fclose(output))error=15;
    free(xml);VirtualFree(image,0,MEM_RELEASE);
    printf("materials=%u mechanics=%u numeric_adapter=%s error=%u\n",count,valid,adapted?"true":"false",error);return error;
}
