/* Whole original four-wheel contact consumer with real source-built world queries.
   Mechanics are uniform or supplied in source order and mapped by original raw resolver. */
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
typedef void (__fastcall *InitializeContacts)(void *,void *,int);
typedef unsigned *(__fastcall *MaterialLookup)(void *,void *,unsigned);
typedef struct { __declspec(align(16)) unsigned char data[0xb0]; } SurfaceRecord;
#ifdef GRID_SUSPENSION_PREPARE
#include "suspension_prepare_probe.h"
#endif
int wmain(int argc,wchar_t **argv){
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=5&&argc!=6)return 2;
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
    unsigned fixture_cursor=24,totalcells=0;
    for(unsigned i=0;i<1024;++i){
        if(!inside(fixture_cursor,8,(size_t)size))return 11;exponents[i]=getword(raw+fixture_cursor);widths[i]=getword(raw+fixture_cursor+4);fixture_cursor+=8;
        if(exponents[i]>6||widths[i]!=(1u<<exponents[i]))return 12;
        unsigned cells=widths[i]*widths[i];totalcells+=cells;if(totalcells>1024*1024||!inside(fixture_cursor,cells*4,(size_t)size))return 13;
        pointers[i]=(unsigned)(uintptr_t)(raw+fixture_cursor);fixture_cursor+=cells*4;
    }
    Object *objects=_aligned_malloc(sizeof(Object)*number,16);if(!objects)return 14;
    for(unsigned i=0;i<number;++i){if(!inside(fixture_cursor,4,(size_t)size))return 15;unsigned length=getword(raw+fixture_cursor);fixture_cursor+=4;if(length>2*1024*1024||!inside(fixture_cursor,length,(size_t)size)||!prepare(raw+fixture_cursor,length,objects+i))return 16;fixture_cursor+=length;}
    if(fixture_cursor!=(unsigned)size)return 17;
    for(unsigned i=0;i<1024;++i){unsigned *cells=(unsigned *)(uintptr_t)pointers[i];for(unsigned j=0;j<widths[i]*widths[i];++j){unsigned id=cells[j];if(id>number)return 18;if(id)cells[j]=(unsigned)(uintptr_t)objects[id-1].wrapper;}}
    unsigned wrapper[3]={0};unsigned char shape[0x248]={0};float header[6]={bounds[0],-1.f,bounds[1],bounds[2],1.f,bounds[3]};wrapper[2]=(unsigned)(uintptr_t)shape;*(unsigned *)(shape+0x244)=(unsigned)(uintptr_t)header;
    *(unsigned *)(image+0xe83af8-0x400000)=(unsigned)(uintptr_t)wrapper;
    /* Dispatch table is read-only; verify its actual type-11 entry. */
    if(*(unsigned *)(image+0xe51fd4+11*4-0x400000)!=(unsigned)(uintptr_t)(image+0xb23800-0x400000))return 19;

    unsigned *source_rows=NULL,source_count=0;SurfaceRecord *source_materials=NULL;
    if(argc==6){
        FILE *mechanics=NULL;char tag[4];
        if(_wfopen_s(&mechanics,argv[5],L"rb")||!mechanics)return 40;
        if(fread(tag,1,4,mechanics)!=4||memcmp(tag,"MEC1",4)||fread(&source_count,4,1,mechanics)!=1||!source_count||source_count>4096)return 41;
        source_rows=malloc(source_count*32);source_materials=_aligned_malloc(source_count*sizeof(SurfaceRecord),16);
        if(!source_rows||!source_materials||fread(source_rows,32,source_count,mechanics)!=source_count||fgetc(mechanics)!=EOF||ferror(mechanics)||fclose(mechanics))return 42;
        memset(source_materials,0,source_count*sizeof(SurfaceRecord));
        for(unsigned i=0;i<source_count;++i){
            unsigned *row=source_rows+i*8;if(row[1]>1||row[2]>1)return 43;
            float parameters[5];memcpy(parameters,row+3,20);for(unsigned j=0;j<5;++j)if(!isfinite(parameters[j]))return 44;
            source_materials[i].data[0x90]=(unsigned char)row[2];memcpy(source_materials[i].data+0x94,row+3,20);
        }
        unsigned key_table[5]={0,(unsigned)(uintptr_t)source_rows,source_count,32,1};
        MaterialLookup lookup=(MaterialLookup)(image+0xb3b0c0-0x400000);
        for(unsigned i=0;i<number;++i)for(unsigned j=0;j<*(unsigned *)(objects[i].header+0x20);++j){
            unsigned *slot=(unsigned *)(objects[i].header+0x34+j*4);unsigned *found=lookup(key_table,NULL,*slot);
            if(!found||found<source_rows||found>=source_rows+source_count*8||(found-source_rows)%8)return 45;
            unsigned index=(unsigned)(found-source_rows)/8;if(!source_rows[index*8+1])return 46;
            *slot=(unsigned)(uintptr_t)source_materials[index].data;
        }
    }

    unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 37;
#ifdef GRID_SUSPENSION_PREPARE
    *cache=0;
    *(unsigned *)(image+0xe85b1c-0x400000)=number;
    unsigned count=0,error=run_suspension_prepare(image,argv[3],argv[4],source_materials,source_count,&count);
#else
    FILE *input=NULL;
    if(_wfopen_s(&input,argv[3],L"r")||!input)return 4;
    HANDLE handle=CreateFileW(argv[4],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){fclose(input);return 5;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);fclose(input);return 6;}
    FILE *output=_fdopen(descriptor,"w");
    if(!output){_close(descriptor);fclose(input);return 7;}
    InitializeContacts initialize=(InitializeContacts)(image+0x9c1660-0x400000);
    char line[8192],name[64];unsigned count=0,error=0;
    while(fgets(line,sizeof(line),input)) {
        if(++count>4096||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;
        while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);
        if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        __declspec(align(16)) float values[166];
        for(unsigned i=0;i<166;++i) {
            while(*cursor=='\t'||*cursor==' ')++cursor;
            char *stop;unsigned long word=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}
            uint32_t word32=(uint32_t)word;memcpy(values+i,&word32,4);cursor=stop;
            if(!isfinite(values[i])){error=11;break;}
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor){error=12;break;}
        __declspec(align(16)) unsigned char body[0x1f00]={0},context[0x740]={0};
        memcpy(body+0x15e0,values,64);
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;
        *(unsigned *)(context+0x5d8)=4;
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=body+0x1990+w*0x150;
            memcpy(context+0x20+w*0x170,values+16+w*4,16);
            memcpy(context+0x30+w*0x170,values+60+w*4,16);
            *(unsigned *)(context+0x178+w*0x170)=(unsigned)(uintptr_t)wheel;
            memcpy(body+0x1650+w*16,values+32+w*4,16);
            memcpy(wheel+0xe0,values+48+w,4);
            memcpy(wheel+0x48,values+52+w,4);
            memcpy(wheel+0x4c,values+56+w,4);
        }
        __declspec(align(16)) unsigned char material[0xb0]={0},table[0x20]={0};
        unsigned entry[3]={0};
        material[0x90]=(unsigned char)(values[124]!=0.f);
        memcpy(material+0x94,values+125,20);
        *(unsigned *)(table+0xc)=1;
        *(unsigned *)(table+0x14)=(unsigned)(uintptr_t)entry;
        *(unsigned *)(context+0x608)=(unsigned)(uintptr_t)table;
        memcpy(body+0x1648,values+130,4);memcpy(body+0x159c,values+131,4);memcpy(body+0x12ac,values+132,4);
        for(unsigned w=0;w<4;++w){
            unsigned char *wheel=body+0x1990+w*0x150;
            memcpy(wheel+0x110,values+76+w,4);
            memcpy(body+0x30c+w*40,values+80+w*10,40);
            memcpy(wheel+0x140,values+120+w,4);
            memcpy(context+0x7c+w*0x170,values+158+w,4);
            memcpy(context+0x80+w*0x170,values+162+w,4);
        }
        if(!source_materials)for(unsigned i=0;i<number;++i)for(unsigned j=0;j<*(unsigned *)(objects[i].header+0x20);++j)*(unsigned *)(objects[i].header+0x34+j*4)=(unsigned)(uintptr_t)material;
        *(unsigned *)(image+0xe85b1c-0x400000)=number;*cache=0;
        __try {initialize(context,NULL,values[157]!=0.f);}
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(output,"F\t%s\t%08lx\n",name,GetExceptionCode());error=13;break;}
        fprintf(output,"P\t%s",name);
        for(unsigned w=0;w<4;++w){
            fprintf(output,"\t%08x\t%08x\t%08x",*(unsigned *)(context+0x68+w*0x170),*(unsigned *)(context+0x80+w*0x170),*(unsigned *)(context+0x7c+w*0x170));
            for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+0xd0+j*4));
            for(unsigned j=0;j<4;++j)fprintf(output,"\t%08x",*(unsigned *)(context+0x40+w*0x170+j*4));
            for(unsigned j=0;j<3;++j)fprintf(output,"\t%08x",*(unsigned *)(body+0x1990+w*0x150+0xc0+j*4)!=0);
            if(source_materials){
                unsigned token=*(unsigned *)(body+0x1990+w*0x150+0xc0),index=0;
                if(token){
                    uintptr_t base=(uintptr_t)source_materials;
                    if(token<base||token>=base+source_count*sizeof(SurfaceRecord)||(token-base)%sizeof(SurfaceRecord)){error=47;break;}
                    index=(unsigned)((token-base)/sizeof(SurfaceRecord))+1;
                }
                fprintf(output,"\t%08x",index);
            }
        }
        fputc('\n',output);
    }
    if(ferror(input)||ferror(output))error=14;
    fclose(input);if(fclose(output))error=15;
#endif
    if(!VirtualProtect(image+0xe81000-0x400000,0x5000,oldgrid,&ignored)||!VirtualProtect(image+0xec2000-0x400000,0x1000,oldcache,&ignored)||!VirtualProtect(image+0xe58000-0x400000,0x1000,oldnormal,&ignored))error=36;
    _aligned_free(source_materials);free(source_rows);_aligned_free(objects);free(raw);VirtualFree(image,0,MEM_RELEASE);printf("track_contact_cases=%u error=%u\n",count,error);return error;
}
