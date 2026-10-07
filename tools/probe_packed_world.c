/* Whole original world collector/dispatch/packed query over a supplied private world. */
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
typedef unsigned (__cdecl *WorldQuery)(float *,float *,float *,float *,unsigned *);
typedef int (__cdecl *Collect)(unsigned *,int,float,float,float,float);
typedef struct {
    __declspec(align(16)) unsigned char shape[0x280];
    unsigned char header[0x60];
    unsigned wrapper[4];
} Object;
static int prepare(unsigned char *chunk,unsigned size,Object *object){
    if(size<52)return 0;int counts[3];memcpy(counts,chunk+24,12);
    if(counts[0]>=0||counts[0]<-32768||counts[1]>=0||counts[1]<-1024||counts[2]>=0||counts[2]<-8)return 0;
    unsigned nt=(unsigned)-counts[0],nv=(unsigned)-counts[1],nm=(unsigned)-counts[2];
    unsigned vo=getword(chunk+36),no=getword(chunk+40),to=getword(chunk+44),ro=getword(chunk+48);
    if(vo!=52+nm*4||no!=vo+nv*8||to<no||(to-no)%2||ro!=to+nt*4||ro>size)return 0;
    unsigned nodes=(to-no)/2,visits=0;unsigned char *active=calloc(nodes,1);if(!active||!validate_tree(chunk,no,nodes,ro,size,nt,0,0,active,&visits)){free(active);return 0;}free(active);
    for(unsigned i=0;i<nt;++i){unsigned char *t=chunk+to+i*4;unsigned first=t[0]*16+(t[1]>>4);if(t[0]>=64||first+t[2]>=nv||first+t[3]>=nv||(unsigned)(t[1]&15)>=nm)return 0;}
    float bounds[6];memcpy(bounds,chunk,24);for(unsigned i=0;i<6;++i)if(!isfinite(bounds[i]))return 0;
    for(unsigned i=0;i<3;++i)if(!(bounds[i]<bounds[i+3])||!isfinite(bounds[i+3]-bounds[i]))return 0;
    memset(object,0,sizeof(*object));memcpy(object->header,chunk,24);*(unsigned *)(object->shape+0x10)=11;
    *(unsigned *)(object->shape+0x244)=(unsigned)(uintptr_t)object->header;
    object->wrapper[2]=(unsigned)(uintptr_t)object->shape;
    unsigned offsets[4]={vo,no,to,ro};for(unsigned i=0;i<4;++i)*(unsigned *)(object->header+0x24+i*4)=(unsigned)(uintptr_t)(chunk+offsets[i]);
    for(unsigned i=0;i<3;++i){*(unsigned *)(object->header+0x18+i*4)=(unsigned)-counts[i];*(float *)(object->shape+0x250+i*4)=(bounds[i+3]-bounds[i])/(i==1?65536.f:16777216.f);*(float *)(object->shape+0x260+i*4)=bounds[i];}
    *(float *)(object->shape+0x25c)=1.f;memcpy(object->header+0x34,chunk+52,nm*4);return 1;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    DWORD oldgrid,oldcache,oldnormal,ignored;
    if(!VirtualProtect(image+0xe81000-0x400000,0x5000,PAGE_READWRITE,&oldgrid)||!VirtualProtect(image+0xec2000-0x400000,0x1000,PAGE_READWRITE,&oldcache)||!VirtualProtect(image+0xe58000-0x400000,0x1000,PAGE_READWRITE,&oldnormal))return 4;
    FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 5;
    long size;if(fseek(file,0,SEEK_END)||(size=ftell(file))<24||size>16*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 6;}
    unsigned char *raw=malloc((size_t)size);if(!raw)return 7;
    int ok=fread(raw,1,(size_t)size,file)==(size_t)size;fclose(file);if(!ok||memcmp(raw,"WQG1",4))return 8;
    float bounds[4];memcpy(bounds,raw+4,16);for(unsigned i=0;i<4;++i)if(!isfinite(bounds[i]))return 9;
    if(bounds[0]>=bounds[2]||bounds[1]>=bounds[3])return 9;
    unsigned number=getword(raw+20);if(!number||number>512)return 10;
    unsigned *widths=(unsigned *)(image+0xe81af8-0x400000),*exponents=(unsigned *)(image+0xe82af8-0x400000),*pointers=(unsigned *)(image+0xe84af8-0x400000),*cache=(unsigned *)(image+0xec2c54-0x400000);
    unsigned cursor=24,totalcells=0;
    for(unsigned i=0;i<1024;++i){
        if(!inside(cursor,8,(size_t)size))return 11;exponents[i]=getword(raw+cursor);widths[i]=getword(raw+cursor+4);cursor+=8;
        if(exponents[i]>6||widths[i]!=(1u<<exponents[i]))return 12;
        unsigned cells=widths[i]*widths[i];totalcells+=cells;if(totalcells>1024*1024||!inside(cursor,cells*4,(size_t)size))return 13;
        pointers[i]=(unsigned)(uintptr_t)(raw+cursor);cursor+=cells*4;
    }
    Object *objects=_aligned_malloc(sizeof(Object)*number,16);if(!objects)return 14;
    for(unsigned i=0;i<number;++i){if(!inside(cursor,4,(size_t)size))return 15;unsigned length=getword(raw+cursor);cursor+=4;if(length>2*1024*1024||!inside(cursor,length,(size_t)size)||!prepare(raw+cursor,length,objects+i))return 16;cursor+=length;}
    if(cursor!=(unsigned)size)return 17;
    for(unsigned i=0;i<1024;++i){unsigned *cells=(unsigned *)(uintptr_t)pointers[i];for(unsigned j=0;j<widths[i]*widths[i];++j){unsigned id=cells[j];if(id>number)return 18;if(id)cells[j]=(unsigned)(uintptr_t)objects[id-1].wrapper;}}
    unsigned wrapper[3]={0};unsigned char shape[0x248]={0};float header[6]={bounds[0],-1.f,bounds[1],bounds[2],1.f,bounds[3]};wrapper[2]=(unsigned)(uintptr_t)shape;*(unsigned *)(shape+0x244)=(unsigned)(uintptr_t)header;
    *(unsigned *)(image+0xe83af8-0x400000)=(unsigned)(uintptr_t)wrapper;
    /* Dispatch table is read-only; verify its actual type-11 entry. */
    if(*(unsigned *)(image+0xe51fd4+11*4-0x400000)!=(unsigned)(uintptr_t)(image+0xb23800-0x400000))return 19;
    WorldQuery query=(WorldQuery)(image+0x96b7f0-0x400000);Collect collect=(Collect)(image+0x96b440-0x400000);
    FILE *input=NULL;if(_wfopen_s(&input,argv[3],L"r")||!input)return 20;
    HANDLE h=CreateFileW(argv[4],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)return 21;
    int descriptor=_open_osfhandle((intptr_t)h,_O_TEXT);if(descriptor<0){CloseHandle(h);return 22;}FILE *output=_fdopen(descriptor,"w");if(!output){_close(descriptor);return 23;}
    char line[2048],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)){
        if(++count>4096||!strchr(line,'\n')){error=24;break;}char *c=line,*begin=c;while(*c&&*c!='\t')++c;size_t n=(size_t)(c-begin);if(!n||n>=sizeof(name)){error=25;break;}memcpy(name,begin,n);name[n]=0;
        unsigned words[10];for(unsigned i=0;i<10;++i){while(*c=='\t'||*c==' ')++c;char *end;words[i]=strtoul(c,&end,16);if(end-c!=8){error=26;break;}c=end;}while(*c=='\t'||*c==' '||*c=='\r'||*c=='\n')++c;if(error||*c||words[8]>3||words[9]>1){error=27;break;}
        __declspec(align(16)) float values[8],normal[4]={0};memcpy(values,words,32);for(unsigned i=0;i<8;++i)if(!isfinite(values[i]))error=28;if(error)break;
        float low[2]={fminf(values[0],values[4]),fminf(values[2],values[6])},high[2]={fmaxf(values[0],values[4]),fmaxf(values[2],values[6])};
        for(unsigned a=0;a<2;++a)for(unsigned j=0;j<2;++j){float point=j?high[a]:low[a];float q=((point-bounds[a])/(bounds[a+2]-bounds[a]))*65536.f;if(!isfinite(q)||q<-2147483648.f||q>=2147483648.f)error=29;}if(error)break;
        *(unsigned *)(image+0xe85b1c-0x400000)=words[9]?number:0;*cache=words[8];float fraction=-9.f;unsigned material=0,result=0,ids[25]={0};int candidates=0;
        __try{if(words[9])candidates=collect(ids,25,low[0],low[1],high[0],high[1]);result=query(values,values+4,normal,&fraction,&material);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=30;break;}
        unsigned selected=0;if(result){for(unsigned i=0;i<number;++i)if(result==(unsigned)(uintptr_t)objects[i].wrapper)selected=i+1;if(!selected){error=31;break;}}
        if(selected){fprintf(output,"H\t%s\t%08x\t%08x",name,selected,bits(fraction));for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",bits(normal[i]));fprintf(output,"\t%08x",material);}else fprintf(output,"N\t%s",name);
        if(candidates<0||candidates>25){error=32;break;}fprintf(output,"\t%08x\t%08x",*cache,candidates);
        for(int j=0;j<candidates;++j){unsigned id=0;for(unsigned i=0;i<number;++i)if(ids[j]==(unsigned)(uintptr_t)objects[i].wrapper)id=i+1;if(!id){error=33;break;}fprintf(output,"\t%08x",id);}fputc('\n',output);if(error)break;
    }
    if(ferror(input)||ferror(output))error=34;if(fclose(input)||fclose(output))error=35;
    if(!VirtualProtect(image+0xe81000-0x400000,0x5000,oldgrid,&ignored)||!VirtualProtect(image+0xec2000-0x400000,0x1000,oldcache,&ignored)||!VirtualProtect(image+0xe58000-0x400000,0x1000,oldnormal,&ignored))error=36;
    _aligned_free(objects);free(raw);VirtualFree(image,0,MEM_RELEASE);printf("packed_world_cases=%u error=%u\n",count,error);return error;
}
