/* Whole original registered contact-event consumer and builder, owned observers.
   Observer callbacks are authored measurement endpoints, not car damage handlers. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include "isolated_reference.h"

enum { MAGIC=0x31454342, CONTACT=0x80, SHAPE=0x220, REGISTRATION=0x20, EVENT=0x50 };
typedef struct { unsigned function,user,word,parameter; unsigned char event[EVENT]; } Observation;
typedef struct { unsigned char contact[CONTACT]; unsigned words[9]; } Request;
static Observation observations[2];
static unsigned observed,observer_error,mutate;
static unsigned char *registrations[2];

static void record(unsigned function,void *event,float parameter,unsigned word,unsigned user) {
    if(observed>=2 || ((uintptr_t)event&15)) { observer_error=1;return; }
    Observation *o=&observations[observed++];
    o->function=function;o->user=user;o->word=word;o->parameter=bits(parameter);
    memcpy(o->event,event,EVENT);
    for(unsigned i=0;i<2;i++) {
        void *p=*(void **)(o->event+4*i);unsigned token=0;
        if(p) {
            if(p==registrations[0])token=1;
            else if(p==registrations[1])token=2;
            else {observer_error=2;return;}
        }
        memcpy(o->event+4*i,&token,4);
    }
    if(mutate) *(unsigned *)((unsigned char *)event+8)^=0xa5a55a5a;
}
static void __cdecl observer_one(void *event,float parameter,unsigned word,unsigned user) {record(1,event,parameter,word,user);}
static void __cdecl observer_two(void *event,float parameter,unsigned word,unsigned user) {record(2,event,parameter,word,user);}
typedef void (__cdecl *Observer)(void *,float,unsigned,unsigned);
typedef void (__stdcall *Consumer)(void *,unsigned,unsigned);

int wmain(int argc,wchar_t **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
    if(argc!=4)return 2;
    FILE *input=NULL,*output=NULL;
    unsigned magic=0,total=0,error=0,completed=0;
    Request *requests=NULL;
    unsigned char *image=NULL,*snapshot=NULL;
    size_t image_size=0;
    if(_wfopen_s(&input,argv[2],L"rb")||!input)return 3;
    if(fread(&magic,4,1,input)!=1||magic!=MAGIC||fread(&total,4,1,input)!=1||!total||total>4096) {error=4;goto finish;}
    requests=calloc(total,sizeof(*requests));
    if(!requests) {error=5;goto finish;}
    if(fread(requests,sizeof(*requests),total,input)!=total||fgetc(input)!=EOF||ferror(input)) {error=6;goto finish;}
    for(unsigned row=0;row<total;row++) {
        unsigned *w=requests[row].words;
        if(w[0]>2||w[1]>2||w[2]>2||w[3]>2||w[8]>1) {error=7;goto finish;}
    }
    image=map_reference(argv[1]);if(!image) {error=8;goto finish;}
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    image_size=pe->OptionalHeader.SizeOfImage;snapshot=malloc(image_size);
    if(!snapshot) {error=5;goto finish;}memcpy(snapshot,image,image_size);
    if(_wfopen_s(&output,argv[3],L"wbx")||!output) {error=9;goto finish;}
    fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
    for(unsigned row=0;row<total;row++) {
        __declspec(align(16)) unsigned char contact[32+CONTACT+32],shapes[2][32+SHAPE+32],regs[2][32+REGISTRATION+32];
        unsigned char before_contact[sizeof(contact)],before_shapes[sizeof(shapes)],before_regs[sizeof(regs)];
        unsigned *w=requests[row].words;
        memset(contact,0xcd,sizeof(contact));memcpy(contact+32,requests[row].contact,CONTACT);
        memset(shapes,0xcd,sizeof(shapes));memset(regs,0xcd,sizeof(regs));
        registrations[0]=regs[0]+32;registrations[1]=regs[1]+32;
        for(unsigned side=0;side<2;side++) {
            Observer callback=w[2+side]==1?observer_one:w[2+side]==2?observer_two:NULL;
            *(void **)(contact+32+0x28+side*4)=shapes[side]+32;
            *(void **)(shapes[side]+32+0x21c)=w[side]?registrations[w[side]-1]:NULL;
            memcpy(regs[side]+32+0xc,&callback,4);memcpy(regs[side]+32+0x10,w+4+side,4);
        }
        memcpy(before_contact,contact,sizeof(contact));memcpy(before_shapes,shapes,sizeof(shapes));memcpy(before_regs,regs,sizeof(regs));
        memset(observations,0,sizeof(observations));observed=observer_error=0;mutate=w[8];
        __try {((Consumer)(image+0xa726b0-0x400000))(contact+32,w[6],w[7]);}
        __except(EXCEPTION_EXECUTE_HANDLER) {fprintf(stderr,"native_exception=%08lx row=%u\n",GetExceptionCode(),row);error=10;}
        if(error)break;
        if(observer_error||memcmp(contact,before_contact,sizeof(contact))||memcmp(shapes,before_shapes,sizeof(shapes))||memcmp(regs,before_regs,sizeof(regs))) {error=11;break;}
        fwrite(&observed,4,1,output);fwrite(observations,sizeof(observations),1,output);completed++;
    }
finish:
    if(image&&snapshot&&memcmp(image,snapshot,image_size))error=12;
    if(input) {int bad=ferror(input);if(fclose(input)||bad)error=13;}
    if(output) {int bad=ferror(output);if(fclose(output)||bad)error=13;}
    free(requests);free(snapshot);if(image)VirtualFree(image,0,MEM_RELEASE);
    printf("body_event_cases=%u error=%u complete_image_unchanged=%u owned_contexts_unchanged=%u observer_error=%u no_imports=1\n",completed,error,error!=12,error!=11,observer_error);
    return (int)error;
}
