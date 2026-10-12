/* Bounded owned packed-source shape; material references supplied separately. */
#ifndef GRID_OWNED_PACKED_SHAPE_PROBE_H
#define GRID_OWNED_PACKED_SHAPE_PROBE_H
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
#endif
