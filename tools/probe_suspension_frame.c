/* Joined original coupling, wheel substeps and finalizer. Supplied geometry,
   extra tyre axes inactive; scheduling/rate application authored from contract.
   No live injection, entry/import/TLS execution or helper replacements. */
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
typedef void (__fastcall *Stage)(void *,void *);
typedef void (__fastcall *Kernel)(void *,void *,float,float);
static void emit(FILE *f,const void *p,unsigned offset,unsigned count) {
    for(unsigned i=0;i<count;i++){unsigned v;memcpy(&v,(const unsigned char *)p+offset+4*i,4);fprintf(f,"\t%08x",v);}
}
int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    unsigned char *image=map_reference(argv[1]);if(!image)return 3;
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *saved_image=malloc(pe->OptionalHeader.SizeOfImage);if(!saved_image)return 4;
    memcpy(saved_image,image,pe->OptionalHeader.SizeOfImage);
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};
    DWORD protections[4];unsigned protected=0,error=0,count=0,total_frames=0;
    for(unsigned i=0;i<4;i++){
        if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,protections+i)){error=5;break;}
        ++protected;
    }
    FILE *input=NULL,*output=NULL;
    if(error||_wfopen_s(&input,argv[2],L"r")||!input){error=6;goto finish;}
    HANDLE handle=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
    if(handle==INVALID_HANDLE_VALUE){error=7;goto finish;}
    int descriptor=_open_osfhandle((intptr_t)handle,_O_TEXT);
    if(descriptor<0){CloseHandle(handle);error=7;goto finish;}
    output=_fdopen(descriptor,"w");if(!output){_close(descriptor);error=7;goto finish;}
    Stage prepare=(Stage)(image+0x9e3970-0x400000),finalize=(Stage)(image+0x9e4b10-0x400000);
    Kernel kernel=(Kernel)(image+0x9c1f60-0x400000);
    char line[4096],name[64];
    while(fgets(line,sizeof(line),input)) {
        if(++count>256||!strchr(line,'\n')){error=8;break;}
        char *cursor=line,*start=cursor;while(*cursor&&*cursor!='\t'&&*cursor!=' ')++cursor;
        size_t length=(size_t)(cursor-start);if(!length||length>=sizeof(name)){error=9;break;}
        memcpy(name,start,length);name[length]=0;
        unsigned words[182];float values[181];
        for(unsigned i=0;i<182;i++){
            while(*cursor=='\t'||*cursor==' ')++cursor;char *stop;words[i]=strtoul(cursor,&stop,16);
            if(stop-cursor!=8){error=10;break;}cursor=stop;
        }
        while(*cursor=='\t'||*cursor==' '||*cursor=='\r'||*cursor=='\n')++cursor;
        if(error||*cursor||!words[0]||words[0]>120){error=11;break;}
        memcpy(values,words+1,sizeof(values));
        for(unsigned i=0;i<181;i++)if(!isfinite(values[i]))error=12;
        if(error||values[0]<=0||values[28]<=0||values[28]>.25f||values[29]<=0){error=12;break;}
        for(unsigned i=0;i<3;i++)if(values[1+i]<=0||values[30+i]<=0)error=12;
        if(error)break;
        __declspec(align(16)) unsigned char body[0x2600]={0},context[0x750]={0};
        unsigned char *state=body+0x15a0;
        memcpy(state,values,4);memcpy(state+8,values+1,12);memcpy(state+0x40,values+4,64);
        memcpy(state+0x20,values+20,16);memcpy(state+0x30,values+24,16);
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;*(unsigned *)(context+0x5d8)=4;
        *(float *)context=values[29];memcpy(context+0x6f0,values+30,12);
        for(unsigned i=0;i<4;i++){
            float *v=values+41+i*35;unsigned char *c=context+i*0x170,*w=body+0x1990+i*0x150;
            memcpy(c+0x10,v,16);memcpy(c+0x30,v+4,16);memcpy(c+0x40,v+8,16);
            *(float *)(c+0x64)=v[12];*(float *)(c+0x74)=v[13];*(float *)(c+0x60)=v[14];
            const unsigned offsets[]={0x5c,0x54,0x4c,0x60,0x64,0x68,0x114,0x58,0x70};
            for(unsigned j=0;j<9;j++)*(float *)(w+offsets[j])=v[14+j];
            *(float *)(context+0x640+i*4)=v[23];*(float *)(c+0x68)=v[24];*(float *)(c+0x78)=v[25];
            *(float *)(c+0x168)=v[26];*(float *)(c+0x16c)=v[27];*(float *)(c+0x174)=v[28];*(float *)(c+0x170)=v[29];
            *(float *)(w+0x48)=v[30];*(float *)(w+0x50)=v[31];*(float *)(w+0x138)=v[33];
            if((v[34]!=0&&v[34]!=1)||bits(v[16])!=bits(v[32])){error=12;break;}c[0x84]=(unsigned char)v[34];
            *(unsigned *)(c+0x178)=(unsigned)(uintptr_t)w;
        }
        if(error)break;
        unsigned steps=(unsigned)(int)(values[28]*(*(float *)(image+0xd18d28-0x400000)));if(!steps)steps=1;
        float decrement=1.f/(float)steps,delta=decrement*values[28];
        for(unsigned frame=0;frame<words[0];frame++) {
            float weight=1.f,last_weight=1.f;
            float coupling[36],accumulated[40];
            __try {
                prepare(context,NULL);
                for(unsigned i=0;i<4;i++)memcpy(coupling+i,context+i*0x170+0x70,4);
                for(unsigned i=0;i<4;i++)memcpy(coupling+4+i*4,context+i*0x170+0x88,16);
                memcpy(coupling+20,context+0x650,64);
                *(float *)(context+0x610)=values[28];
                for(unsigned i=0;i<steps;i++){last_weight=weight;kernel(context,NULL,delta,weight);weight-=decrement;}
                const unsigned fields[]={0x64,0x74,0x70,0x68,0x78,0x168,0x16c,0x174,0x170};
                for(unsigned i=0;i<4;i++){
                    for(unsigned j=0;j<9;j++)memcpy(accumulated+i*10+j,context+i*0x170+fields[j],4);
                    accumulated[i*10+9]=context[i*0x170+0x84]?1.f:0.f;
                }
                finalize(context,NULL);
                for(unsigned j=0;j<4;j++){
                    float *linear=(float *)(state+0x20),*angular=(float *)(state+0x30),*position=(float *)(state+0x70);
                    linear[j]=values[28]*values[33+j]+linear[j];angular[j]=values[28]*values[37+j]+angular[j];
                    position[j]=(values[28]*values[33+j])*(values[28]*.5f)+position[j];
                }
            } __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"case=%s frame=%u fault=%08lx\n",name,frame,GetExceptionCode());error=13;}
            if(error)break;
            fprintf(output,"P\t%s\t%u\t%u\t%08x\t%08x",name,frame,steps,bits(delta),bits(last_weight));
            emit(output,coupling,0,36);emit(output,accumulated,0,40);
            emit(output,state,0,1);emit(output,state,8,3);emit(output,state,0x40,16);emit(output,state,0x20,8);
            const unsigned fields[]={0x40,0x3c,0x44,0x138},accums[]={0x168,0x16c,0x174,0x170};
            for(unsigned i=0;i<4;i++){
                for(unsigned j=0;j<4;j++)emit(output,body+0x1990+i*0x150,fields[j],1);
                for(unsigned j=0;j<4;j++)emit(output,context+i*0x170,accums[j],1);
            }
            emit(output,context,0x610,1);fputc('\n',output);++total_frames;
        }
        if(error)break;
    }
finish:
    for(unsigned i=0;i<protected;i++){
        unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved_image+pages[i]-0x400000,4096);
        DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=14;
    }
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){
        size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;
        if(memcmp(saved_image+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=15;
    }
    if(input&&ferror(input))error=16;if(output&&ferror(output))error=16;
    if(input&&fclose(input))error=16;if(output&&fclose(output))error=16;
    free(saved_image);VirtualFree(image,0,MEM_RELEASE);
    printf("suspension_frame_cases=%u frames=%u error=%u code_unchanged=%u\n",count,total_frames,error,error!=15);return (int)error;
}
