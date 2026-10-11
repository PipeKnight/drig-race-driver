/* Bounded binary-XML view builder shared by private source-loader probes. */
#ifndef GRID_SURFACE_XML_PROBE_H
#define GRID_SURFACE_XML_PROBE_H
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
#endif
