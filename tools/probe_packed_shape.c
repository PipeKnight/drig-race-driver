/* Whole original untransformed packed shape query; supplied source data. */
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
typedef unsigned char (__cdecl *ShapeQuery)(void *,float *,float *,float *,float *,unsigned *,unsigned *);
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
    DWORD old,oldnormal,ignored;if(!VirtualProtect(image+0xec2000-0x400000,0x1000,PAGE_READWRITE,&old))return 27;
    if(!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&oldnormal))return 30;
    unsigned *cache=(unsigned *)(image+0xec2c54-0x400000);ShapeQuery query=(ShapeQuery)(image+0xb23800-0x400000);
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
    for(unsigned i=0;i<3;++i){*(float *)(shape+0x250+i*4)=(bounds[i+3]-bounds[i])/(i==1?65536.f:16777216.f);*(float *)(shape+0x260+i*4)=bounds[i];}
    *(float *)(shape+0x25c)=1.f;*(unsigned *)(shape+0xc8)=0xdeadbeef;
    for(unsigned i=0;i<3;++i)*(unsigned *)(header+0x18+i*4)=(unsigned)-counts[i];
    memcpy(header+0x34,chunk+52,nm*4);
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
        __declspec(align(16)) float values[8],normal[4]={0};float fraction=-9.f;unsigned aux=0xabcdef01,material=0,result=0;memcpy(values,words,32);
        for(unsigned i=0;i<8;++i)if(!isfinite(values[i]))error=22;if(error)break;
        __try{result=query(shape,values,values+4,normal,&fraction,&aux,&material);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=23;break;}
        if(result){fprintf(output,"H\t%s\t%08x",name,bits(fraction));for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",bits(normal[i]));fprintf(output,"\t%08x\t%08x\t%08x\n",material,aux,*cache);}
        else fprintf(output,"N\t%s\t%08x\t%08x\t%08x\n",name,material,aux,*cache);
    }
    if(ferror(input)||ferror(output))error=25;if(fclose(input)||fclose(output))error=26;free(chunk);if(!VirtualProtect(image+0xec2000-0x400000,0x1000,old,&ignored))error=29;if(!VirtualProtect(image+0xe58000-0x400000,0x1000,oldnormal,&ignored))error=31;VirtualFree(image,0,MEM_RELEASE);
    printf("packed_shape_cases=%u error=%u\n",count,error);return error;
}
