/* Whole original packed leaf consumer, expansion, decoder and triangle primitive.
   Supplied source chunk/leaf/start/direction; no traversal or live injection. */
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
typedef void (__fastcall *Leaf)(void *,void *,void *,void *);
static unsigned getword(const unsigned char *p){unsigned v;memcpy(&v,p,4);return v;}
static unsigned char *read_chunk(const wchar_t *path,unsigned *length){
    FILE *file=NULL;if(_wfopen_s(&file,path,L"rb")||!file)return NULL;
    long size;if(fseek(file,0,SEEK_END)||(size=ftell(file))<52||size>2*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return NULL;}
    unsigned char *raw=malloc((size_t)size);if(!raw){fclose(file);return NULL;}int ok=fread(raw,1,(size_t)size,file)==(size_t)size;fclose(file);
    if(!ok){free(raw);return NULL;}*length=(unsigned)size;return raw;
}
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    unsigned size=0;unsigned char *chunk=read_chunk(argv[2],&size);if(!chunk)return 4;
    int counts[3];memcpy(counts,chunk+24,12);
    if(counts[0]>=0||counts[0]<-32768||counts[1]>=0||counts[1]<-1024||counts[2]>=0||counts[2]<-8)return 5;
    unsigned nt=(unsigned)-counts[0],nv=(unsigned)-counts[1],nm=(unsigned)-counts[2];
    unsigned vo=getword(chunk+36),no=getword(chunk+40),to=getword(chunk+44),ro=getword(chunk+48);
    if(vo!=52+nm*4||no!=vo+nv*8||to<no||(to-no)%2||ro!=to+nt*4||ro>size)return 6;
    for(unsigned i=0;i<nt;++i){unsigned char *t=chunk+to+i*4;unsigned first=t[0]*16+(t[1]>>4);if(t[0]>=64||first+t[2]>=nv||first+t[3]>=nv||(unsigned)(t[1]&15)>=nm)return 7;}
    float bounds[6];memcpy(bounds,chunk,24);for(unsigned i=0;i<6;++i)if(!isfinite(bounds[i]))return 8;
    for(unsigned i=0;i<3;++i)if(!(bounds[i]<bounds[i+3])||!isfinite(bounds[i+3]-bounds[i]))return 9;
    __declspec(align(16)) unsigned char shape[0x280]={0};unsigned char header[0x60]={0};memcpy(header,chunk,vo<sizeof(header)?vo:sizeof(header));
    *(unsigned *)(shape+0x244)=(unsigned)(uintptr_t)header;
    *(unsigned *)(header+0x24)=(unsigned)(uintptr_t)(chunk+vo);*(unsigned *)(header+0x2c)=(unsigned)(uintptr_t)(chunk+to);*(unsigned *)(header+0x30)=(unsigned)(uintptr_t)(chunk+ro);
    Leaf leaf=(Leaf)(image+0xb23450-0x400000);
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
        unsigned offset=words[0];if(offset>=0x7fff||!inside(ro+offset,3,size)){error=18;break;}
        unsigned p=ro+offset,total=1,index=chunk[p]*256+chunk[p+1];p+=2;if(index>=nt){error=19;break;}
        while(p<size&&chunk[p]!=255){unsigned delta=chunk[p++];index+=delta;if(delta!=254){++total;if(index>=nt)error=20;}}
        if(p==size||total>256||error){error=21;break;}
        __declspec(align(16)) unsigned char context[0x60]={0};float values[8];memcpy(values,words+1,32);for(unsigned i=0;i<8;++i)if(!isfinite(values[i]))error=22;if(error)break;
        memcpy(context+0x10,values,16);memcpy(context+0x30,values+4,16);memcpy(context+0x40,values+4,16);*(float *)(context+0x50)=1.;
        unsigned char node[2]={(unsigned char)(0x80|(offset>>8)),(unsigned char)offset};
        __try{leaf(shape,NULL,node,context);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=23;break;}
        unsigned char *token=*(unsigned char **)(context+0x58);
        if(token){unsigned triangle=(unsigned)(token-(chunk+to))/4;if(triangle>=nt){error=24;break;}
            fprintf(output,"H\t%s\t%08x\t%08x\t%08x",name,triangle,getword(context+0x50),context[0x54]);for(unsigned i=0;i<4;++i)fprintf(output,"\t%08x",getword(context+0x40+i*4));fputc('\n',output);
        }else fprintf(output,"N\t%s\n",name);
    }
    if(ferror(input)||ferror(output))error=25;if(fclose(input)||fclose(output))error=26;free(chunk);VirtualFree(image,0,MEM_RELEASE);
    printf("leaf_query_cases=%u error=%u\n",count,error);return error;
}
