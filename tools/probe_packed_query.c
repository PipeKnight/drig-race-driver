/* Original root cull and whole packed traversal with original subdivision,
   leaf expansion, decoder and primitive. No normal/material/whole-world claim. */
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
typedef void (__fastcall *Traverse)(void *,void *,void *,void *,void *);
typedef unsigned (__cdecl *Cull)(float *,float *,float *,float *,float *,float *,float *,float *);
static unsigned getword(const unsigned char *p){unsigned v;memcpy(&v,p,4);return v;}
static unsigned char *read_chunk(const wchar_t *path,unsigned *length){
    FILE *file=NULL;if(_wfopen_s(&file,path,L"rb")||!file)return NULL;
    long size;if(fseek(file,0,SEEK_END)||(size=ftell(file))<52||size>2*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return NULL;}
    unsigned char *raw=malloc((size_t)size);if(!raw){fclose(file);return NULL;}int ok=fread(raw,1,(size_t)size,file)==(size_t)size;fclose(file);
    if(!ok){free(raw);return NULL;}*length=(unsigned)size;return raw;
}
static int validate_tree(const unsigned char *raw,unsigned no,unsigned nodes,unsigned ro,unsigned size,unsigned nt,unsigned index,unsigned depth,unsigned char *active,unsigned *visits){
    if(index>=nodes||depth>64||++*visits>1000000||active[index])return 0;
    unsigned node=raw[no+index*2]*256+raw[no+index*2+1];
    if(node==65535)return 1;
    if(node&32768){
        unsigned p=ro+(node&32767);if(!inside(p,3,size))return 0;
        unsigned triangle=raw[p]*256+raw[p+1],count=1;p+=2;if(triangle>=nt)return 0;
        while(p<size&&raw[p]!=255){unsigned delta=raw[p++];triangle+=delta;if(delta!=254&&(++count>256||triangle>=nt))return 0;}
        return p<size;
    }
    if(node+4>nodes)return 0;active[index]=1;
    for(unsigned i=0;i<4;++i)if(!validate_tree(raw,no,nodes,ro,size,nt,node+i,depth+1,active,visits)){active[index]=0;return 0;}
    active[index]=0;return 1;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    DWORD old,ignored;if(!VirtualProtect(image+0xec2000-0x400000,0x1000,PAGE_READWRITE,&old))return 27;
    unsigned *cache=(unsigned *)(image+0xec2c54-0x400000);Cull cull=(Cull)(image+0xadb910-0x400000);
    unsigned size=0;unsigned char *chunk=read_chunk(argv[2],&size);if(!chunk)return 4;
    int counts[3];memcpy(counts,chunk+24,12);
    if(counts[0]>=0||counts[0]<-32768||counts[1]>=0||counts[1]<-1024||counts[2]>=0||counts[2]<-8)return 5;
    unsigned nt=(unsigned)-counts[0],nv=(unsigned)-counts[1],nm=(unsigned)-counts[2];
    unsigned vo=getword(chunk+36),no=getword(chunk+40),to=getword(chunk+44),ro=getword(chunk+48);
    if(vo!=52+nm*4||no!=vo+nv*8||to<no||(to-no)%2||ro!=to+nt*4||ro>size)return 6;
    unsigned node_count=(to-no)/2,visits=0;unsigned char *active=calloc(node_count,1);if(!active||!validate_tree(chunk,no,node_count,ro,size,nt,0,0,active,&visits)){free(active);return 28;}free(active);
    for(unsigned i=0;i<nt;++i){unsigned char *t=chunk+to+i*4;unsigned first=t[0]*16+(t[1]>>4);if(t[0]>=64||first+t[2]>=nv||first+t[3]>=nv||(unsigned)(t[1]&15)>=nm)return 7;}
    float bounds[6];memcpy(bounds,chunk,24);for(unsigned i=0;i<6;++i)if(!isfinite(bounds[i]))return 8;
    for(unsigned i=0;i<3;++i)if(!(bounds[i]<bounds[i+3])||!isfinite(bounds[i+3]-bounds[i]))return 9;
    __declspec(align(16)) unsigned char shape[0x280]={0};unsigned char header[0x60]={0};memcpy(header,chunk,vo<sizeof(header)?vo:sizeof(header));
    *(unsigned *)(shape+0x244)=(unsigned)(uintptr_t)header;
    *(unsigned *)(header+0x24)=(unsigned)(uintptr_t)(chunk+vo);*(unsigned *)(header+0x2c)=(unsigned)(uintptr_t)(chunk+to);*(unsigned *)(header+0x30)=(unsigned)(uintptr_t)(chunk+ro);
    Traverse traverse=(Traverse)(image+0xb23630-0x400000);
    float rectangle[4]={bounds[0],bounds[2],bounds[3],bounds[5]};
    float center[2]={(rectangle[0]+rectangle[2])*0.5f,(rectangle[1]+rectangle[3])*0.5f};
    float half[2]={(rectangle[2]-rectangle[0])*0.5f,(rectangle[3]-rectangle[1])*0.5f};
    *(unsigned *)(header+0x28)=(unsigned)(uintptr_t)(chunk+no);
    FILE *input=NULL;if(_wfopen_s(&input,argv[3],L"r")||!input)return 10;
    HANDLE h=CreateFileW(argv[4],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)return 11;
    int descriptor=_open_osfhandle((intptr_t)h,_O_TEXT);if(descriptor<0){CloseHandle(h);return 12;}
    FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);return 13;}
    char line[2048],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++count>1024||!strchr(line,'\n')){error=14;break;}
        char *cursor=line,*begin=cursor;while(*cursor&&*cursor!='\t')++cursor;size_t n=(size_t)(cursor-begin);if(!n||n>=sizeof(name)){error=15;break;}memcpy(name,begin,n);name[n]=0;
        unsigned words[9];for(unsigned i=0;i<9;++i){while(*cursor=='\t'||*cursor==' ')++cursor;char *end;words[i]=strtoul(cursor,&end,16);if(end-cursor!=8){error=16;break;}cursor=end;}
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;if(error||*cursor){error=17;break;}
        if(words[8]>3){error=18;break;}*cache=words[8];
        __declspec(align(16)) unsigned char context[0x60]={0};float values[8],delta[4];memcpy(values,words,32);for(unsigned i=0;i<8;++i)if(!isfinite(values[i]))error=22;
        for(unsigned i=0;i<4;++i){delta[i]=values[i+4]-values[i];if(!isfinite(delta[i]))error=22;}if(error)break;
        memcpy(context+0x10,values,16);memcpy(context+0x30,delta,16);memcpy(context+0x40,delta,16);*(float *)(context+0x50)=1.;
        __try{if(cull(values,values+2,delta,delta+2,center,center+1,half,half+1)&255)traverse(shape,NULL,chunk+no,rectangle,context);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=23;break;}
        unsigned char *token=*(unsigned char **)(context+0x58);
        if(token){unsigned triangle=(unsigned)(token-(chunk+to))/4;if(triangle>=nt){error=24;break;}
            fprintf(output,"H\t%s\t%08x\t%08x\t%08x",name,triangle,getword(context+0x50),context[0x54]);for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",getword(context+0x40+i*4));fprintf(output,"\t%08x\n",*cache);
        }else fprintf(output,"N\t%s\t%08x\n",name,*cache);
    }
    if(ferror(input)||ferror(output))error=25;if(fclose(input)||fclose(output))error=26;free(chunk);if(!VirtualProtect(image+0xec2000-0x400000,0x1000,old,&ignored))error=29;VirtualFree(image,0,MEM_RELEASE);
    printf("packed_query_cases=%u error=%u\n",count,error);return error;
}
