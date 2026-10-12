/* Owned selected CQTC descriptor for original kind-7/kind-10 dispatch.
   Whole constructor is validated separately by probe_cqtc_decode.c. */
#ifndef GRID_OWNED_CQTC_SHAPE_PROBE_H
#define GRID_OWNED_CQTC_SHAPE_PROBE_H
#define GRID_PACKED_SHAPE_BYTES 0x280
static unsigned getword(const unsigned char *p){unsigned v;memcpy(&v,p,4);return v;}
static unsigned cq24(const unsigned char *p){return ((unsigned)p[0]<<16)|((unsigned)p[1]<<8)|p[2];}
typedef struct {
 __declspec(align(16)) unsigned char shape[GRID_PACKED_SHAPE_BYTES];
 unsigned char header[0x60];
 unsigned wrapper[4];
} Object;
static int cqtree(const unsigned char *b,unsigned no,unsigned nodes,unsigned ro,unsigned size,unsigned nt,unsigned index,unsigned depth,unsigned char *active,unsigned *visits){
 if(index>=nodes||depth>64||++*visits>1000000||active[index])return 0;
 unsigned n=cq24(b+no+index*3);
 if(n==0xffffff)return 1;
 if(n&0x800000){
  unsigned p=ro+(n&0x7fffff);if(!inside(p,3,size))return 0;
  unsigned first=cq24(b+p),base=first&0x7fffff;p+=3;if(base>=nt)return 0;
  if(first&0x800000)return 1;
  unsigned count=1;
  for(;;){if(!inside(p,2,size)||++count>256)return 0;unsigned delta=((unsigned)b[p]<<8)|b[p+1];p+=2;if(base+(delta&0x7fff)>=nt)return 0;if(delta&0x8000)return 1;}
 }
 if(n+4>nodes)return 0;active[index]=1;
 for(unsigned i=0;i<4;i++)if(!cqtree(b,no,nodes,ro,size,nt,n+i,depth+1,active,visits)){active[index]=0;return 0;}
 active[index]=0;return 1;
}
static int prepare(unsigned char *b,unsigned size,Object *o){
 if(size<56)return 0;
 unsigned nt=getword(b+24),nv=getword(b+28),nm=getword(b+32),vo=getword(b+36),no=getword(b+40),to=getword(b+44),ro=getword(b+48);
 if(!nt||nt>32768||!nv||nv>65535||nm!=1||vo!=56||no!=vo+nv*8||to<=no||to>size||(to-no)%3||ro!=to+nt*7||ro>size)return 0;
 unsigned nodes=(to-no)/3,visits=0;unsigned char *active=calloc(nodes,1);if(!active||!cqtree(b,no,nodes,ro,size,nt,0,0,active,&visits)){free(active);return 0;}free(active);
 for(unsigned i=0;i<nt;i++){const unsigned char *t=b+to+i*7;unsigned a=cq24(t),bb=a+t[4]+((unsigned)(t[3]&240)<<4),c=a+t[5]+((unsigned)(t[3]&15)<<8);if(a>=nv||bb>=nv||c>=nv||t[6])return 0;}
 float bounds[6];memcpy(bounds,b,24);for(unsigned i=0;i<6;i++)if(!isfinite(bounds[i]))return 0;
 for(unsigned i=0;i<3;i++)if(bounds[i]>=bounds[i+3]||!isfinite(bounds[i+3]-bounds[i]))return 0;
 memset(o,0,sizeof(*o));memcpy(o->header,b,24);*(unsigned *)(o->shape+0x10)=10;*(unsigned *)(o->shape+0x244)=(unsigned)(uintptr_t)o->header;
 o->wrapper[2]=(unsigned)(uintptr_t)o->shape;
 unsigned offsets[4]={vo,no,to,ro};for(unsigned i=0;i<4;i++)*(unsigned *)(o->header+0x24+i*4)=(unsigned)(uintptr_t)(b+offsets[i]);
 for(unsigned i=0;i<3;i++){*(unsigned *)(o->header+0x18+i*4)=getword(b+0x18+i*4);*(float *)(o->shape+0x250+i*4)=(bounds[i+3]-bounds[i])/(i==1?65536.f:16777216.f);*(float *)(o->shape+0x260+i*4)=bounds[i];}
 *(float *)(o->shape+0x25c)=1.f;memcpy(o->header+0x34,b+52,4);return 1;
}
#endif
