/* Original physical-material loader and XML accessors. In adapted mode ONLY
   numeric CRT conversions use host strtod/strtol, explicitly excluding original
   CRT initialization/text-parsing equivalence. Original mode leaves all code
   unchanged and is used for defaults/name/boolean controls. All patches restored. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <io.h>
#include <fcntl.h>
#include <wchar.h>
#include "isolated_reference.h"
#include "surface_xml_probe.h"
typedef void (__stdcall *Load)(void *,void *);
typedef struct {unsigned char *entry;unsigned char saved[5];int active;} Patch;
static double __cdecl number_adapter(const char *s){return strtod(s,NULL);}
static long __cdecl integer_adapter(const char *s,char **end,int base){return strtol(s,end,base);}
static int patch(Patch *p,unsigned char *entry,uintptr_t target){
    DWORD old,ignored;p->entry=entry;memcpy(p->saved,entry,5);
    if(!VirtualProtect(entry,5,PAGE_READWRITE,&old))return 0;
    entry[0]=0xe9;unsigned relative=(unsigned)target-(unsigned)(uintptr_t)(entry+5);memcpy(entry+1,&relative,4);p->active=1;
    return VirtualProtect(entry,5,old,&ignored)&&FlushInstructionCache(GetCurrentProcess(),entry,5);
}
static int restore(Patch *p){
    if(!p->active)return 1;DWORD old,ignored;
    if(!VirtualProtect(p->entry,5,PAGE_READWRITE,&old))return 0;
    memcpy(p->entry,p->saved,5);p->active=0;
    return VirtualProtect(p->entry,5,old,&ignored)&&FlushInstructionCache(GetCurrentProcess(),p->entry,5)&&!memcmp(p->entry,p->saved,5);
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=5||(wcscmp(argv[4],L"original")&&wcscmp(argv[4],L"adapted")))return 2;
    int adapted=!wcscmp(argv[4],L"adapted");unsigned count=0,error=0;Patch patches[2]={0};
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    unsigned doc[9]={0};unsigned char *xml=read_xml(argv[2],doc);FILE *output=NULL;
    if(!xml){error=4;goto done;}
    unsigned *root=(unsigned *)(uintptr_t)doc[3];
    if(!doc[2]||strcmp(text(doc,root[0]),"MATERIALS")){error=5;goto done;}
    if(adapted&&(!patch(&patches[0],image+0xbf3b60-0x400000,(uintptr_t)&number_adapter)||
        !patch(&patches[1],image+0xbf630f-0x400000,(uintptr_t)&integer_adapter))){error=6;goto done;}
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){error=7;goto done;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);error=8;goto done;}
    output=_fdopen(descriptor,"w");if(!output){_close(descriptor);error=9;goto done;}
    Load load=(Load)(image+0xa720d0-0x400000);
    for(unsigned i=0;i<root[4];++i){
        unsigned *e=(unsigned *)((unsigned char *)(uintptr_t)doc[3]+(root[5]+i)*24);
        if(strcmp(text(doc,e[0]),"MATERIAL"))continue;
        unsigned view[11]={0};memcpy(view+2,root,24);view[1]=i;view[8]=(unsigned)(uintptr_t)doc;
        __declspec(align(16)) unsigned char material[64];memset(material,0xcd,sizeof(material));
        __try{load(material,view);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"FAULT\t%u\t%08lx\n",i,GetExceptionCode());error=10;break;}
        /* Only byte38 is written; padding39..3f must retain the sentinel. */
        for(unsigned j=0x39;j<sizeof(material);++j)if(material[j]!=0xcd)error=11;
        if(error)break;
        fputs("P",output);for(unsigned j=0;j<14;++j)fprintf(output,"\t%08x",word(material+4*j));
        fprintf(output,"\t%08x\n",material[0x38]);++count;
    }
    if(ferror(output))error=12;
done:
    if(output&&fclose(output))error=13;
    int restored_integer=restore(&patches[1]),restored_number=restore(&patches[0]);
    if(!restored_integer||!restored_number)error=14;
    free(xml);VirtualFree(image,0,MEM_RELEASE);
    printf("materials=%u adapted=%d restored=%d error=%u\n",count,adapted,!patches[0].active&&!patches[1].active,error);
    return (int)error;
}
