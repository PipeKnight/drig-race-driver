/* Owned mesh relocation and pointer normalization for isolated contact probes. */
#ifndef GRID_OWNED_COLLISION_MESH_PROBE_H
#define GRID_OWNED_COLLISION_MESH_PROBE_H
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((char *)p+o,&v,4);}
static float num(const void *p,unsigned o){float v;memcpy(&v,(const char *)p+o,4);return v;}
static unsigned address(const void *p){return (unsigned)(uintptr_t)p;}
static uintptr_t original;
static LONG fault(EXCEPTION_POINTERS *e){fprintf(stderr,"mesh_contact_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-original+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
typedef struct {unsigned char *p;unsigned length,token;} Region;
static int canonical(void *p,unsigned o,const Region *regions,unsigned n){
 unsigned value=get(p,o);if(!value)return 1;
 for(unsigned i=0;i<n;i++)if(value>=address(regions[i].p)&&value-address(regions[i].p)<regions[i].length){put(p,o,regions[i].token+value-address(regions[i].p));return 1;}
 return 0;
}
static int mesh_pointer(unsigned char *mesh,unsigned offset,unsigned length,int native){
 unsigned v=get(mesh,offset),base=address(mesh);
 if(native){if(v>=length||(v&3))return 0;put(mesh,offset,base+v);}
 else {if(v<base||v-base>=length||((v-base)&3))return 0;put(mesh,offset,v-base);}
 return 1;
}
static int relocate_mesh(unsigned char *mesh,unsigned length,int native){
 unsigned points=get(mesh,0),faces=get(mesh,4),face=0x60+points*48;
 for(unsigned i=0;i<points;i++){
  unsigned vertex=0x60+i*32,aux=0x60+points*32+i*16;
  unsigned adjacent=get(mesh,aux+4)-(native?0:address(mesh)),n=get(mesh,aux);
  unsigned incident=get(mesh,vertex+0x18)-(native?0:address(mesh)),t=get(mesh,vertex+0x14);
  if(n>points||t>faces||adjacent>length||n*4>length-adjacent||incident>length||t*4>length-incident)return 0;
  for(unsigned j=0;j<n;j++){
   unsigned v=get(mesh,adjacent+j*4)-(native?0:address(mesh));
   if(v<0x60||v>=0x60+points*32||(v-0x60)%32||!mesh_pointer(mesh,adjacent+j*4,length,native))return 0;
  }
  for(unsigned j=0;j<t;j++){
   unsigned v=get(mesh,incident+j*4)-(native?0:address(mesh));
   if(v<face||v>=face+faces*32||(v-face)%32||!mesh_pointer(mesh,incident+j*4,length,native))return 0;
  }
  if(!mesh_pointer(mesh,vertex+0x10,length,native)||!mesh_pointer(mesh,vertex+0x18,length,native)||!mesh_pointer(mesh,aux+4,length,native))return 0;
 }
 return mesh_pointer(mesh,0x54,length,native)&&mesh_pointer(mesh,0x58,length,native);
}
#endif
