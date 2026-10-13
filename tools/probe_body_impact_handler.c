/* Whole original contact consumer -> registered car callback -> mode0 handler.
   Explicit authored notification observers; no dynamic other-car response. */
#define GRID_COLLISION_DAMAGE_HELPERS
#include "probe_collision_damage.c"
#define PREFIX 52
#define RECORD (PREFIX+CAR+PHYSICAL)
#define TAIL 0x37100
#define PACKET 56
#define RESULT (12+11*PACKET+CAR+PHYSICAL)
static unsigned packets[11][PACKET/4],packet_count,packet_error;
static unsigned char *registered[2],*active_car;
static void __cdecl notification(void *car,unsigned user,void *other,void *point,void *normal,unsigned word,float impact,float parameter){
 if(packet_count>=11||car!=active_car){packet_error=1;return;}
 unsigned *p=packets[packet_count++];p[0]=1;p[1]=user;
 if(!other)p[2]=0;else if(other==registered[0])p[2]=1;else if(other==registered[1])p[2]=2;else{packet_error=2;return;}
 memcpy(p+3,point,16);memcpy(p+7,normal,16);p[11]=word;p[12]=bits(impact);p[13]=bits(parameter);
}
static int handler_admitted(const unsigned char *r){
 unsigned char probe[SIZE];memcpy(probe,r,32);put(probe,4,0);put(probe,12,0);
 if(get(r,8)==0x7f7fffff)put(probe,8,0);
 memcpy(probe+32,r+PREFIX,CAR+PHYSICAL);
 const unsigned char *v=r+PREFIX,*p=v+CAR;
 if(!admitted(probe)||get(r,4)>7||get(r,48)>0x7ff||!range(r,12,0.f,.1f)||v[0x308]||v[0xb13]>1||v[0xb14]>1)return 0;
 for(unsigned j=0;j<4;j++)if(!range(r,32+j*4,-1000.f,1000.f)||!range(p,0x220+j*4,-10000.f,10000.f))return 0;
 return range(v,0x15a0,0.f,1.e9f)&&range(v,0x15a4,0.f,1000.f)&&range(v,0x2e8,-1000.f,1000.f)&&range(v,0x3d0,-1000.f,1000.f);
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[2];if(fread(h,4,2,file)!=2||h[0]!=0x31304842||!h[1]||h[1]>64){fclose(file);return 5;}
 unsigned count=h[1],error=0;unsigned char *inputs=malloc(count*RECORD),*outputs=malloc(8+count*RESULT);
 if(!inputs||!outputs){fclose(file);free(inputs);free(outputs);return 6;}
 int valid=fread(inputs,1,count*RECORD,file)==count*RECORD&&fgetc(file)==EOF;fclose(file);
 if(!valid){free(inputs);free(outputs);return 7;}
 for(unsigned i=0;i<count;i++)if(!handler_admitted(inputs+i*RECORD)){free(inputs);free(outputs);return 8;}
 memcpy(outputs,h,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(inputs);free(outputs);return 9;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *snapshot=malloc(image_size);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return 10;}memcpy(snapshot,image,image_size);
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xe85000,0xe4f000};DWORD prior[6],ignored;unsigned protected=0;
 for(unsigned i=0;i<6;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,prior+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=inputs+i*RECORD;unsigned selector=get(r,4);
  __declspec(align(16)) unsigned char owner[32+TAIL+32],physical[32+0x430+32],component[32+0x100+32],wrapper[32+0x10+32];
  __declspec(align(16)) unsigned char regs[2][32+0x20+32],objects[2][32+0xe0+32],other_body[32+0x430+32],shapes[2][32+0x220+32],contact[32+0x80+32],event[32+0x50+32];
  unsigned char contexts_before[sizeof(regs)+sizeof(objects)+sizeof(other_body)+sizeof(shapes)+sizeof(contact)+sizeof(event)],component_before[sizeof(component)],wrapper_before[sizeof(wrapper)];
  unsigned char *v=owner+32,*p=physical+32,*c=component+32,*w=wrapper+32,*e=event+32,*t=contact+32;active_car=v;
  memset(owner,0xcd,sizeof(owner));memset(v,0,TAIL);memcpy(v,r+PREFIX,CAR);
  memset(physical,0xcd,sizeof(physical));memset(p,0,0x430);memcpy(p,r+PREFIX+CAR,PHYSICAL);
  memset(component,0,sizeof(component));memset(wrapper,0,sizeof(wrapper));
  put(w,8,(unsigned)(uintptr_t)p);put(v,0xdb0,(unsigned)(uintptr_t)w);put(c,0x24,(unsigned)(uintptr_t)v);put(v,MUTABLE+0x214,(unsigned)(uintptr_t)c);put(p,0x424,(unsigned)(uintptr_t)w);
  memcpy(component_before,component,sizeof(component));memcpy(wrapper_before,wrapper,sizeof(wrapper));
  memset(regs,0xcd,sizeof(regs));memset(objects,0xcd,sizeof(objects));memset(other_body,0xcd,sizeof(other_body));memset(shapes,0xcd,sizeof(shapes));memset(contact,0xcd,sizeof(contact));memset(event,0xcd,sizeof(event));
  registered[0]=regs[0]+32;registered[1]=regs[1]+32;
  put(other_body+32,0x424,(unsigned)(uintptr_t)(other_body+32));
  memset(e,0,0x50);memset(t,0,0x80);memcpy(e+0x10,r+16,16);memcpy(e+0x20,r+32,16);memcpy(e+0x40,r+8,4);
  put(e,8,0x12345678);put(e,12,0x9abcdef0);
  for(unsigned side=0;side<2;side++){
   int present=!(selector==2&&side==1)&&!(selector==3&&side==0);
   int match=(selector==2&&side==0)||(selector==3&&side==1)||selector==4||(selector==6&&side==1);
   int null_body=selector==7||(selector==6&&side==0);
   put(e,side*4,present?(unsigned)(uintptr_t)registered[side]:0);
   put(registered[side],8,(unsigned)(uintptr_t)(objects[side]+32));put(objects[side]+32,0xd8,null_body?0:(unsigned)(uintptr_t)(match?p:other_body+32));
   put(registered[side],0xc,(unsigned)(uintptr_t)(image+0xa01910-0x400000));put(registered[side],0x10,(unsigned)(uintptr_t)v);
   put(t,0x28+side*4,(unsigned)(uintptr_t)(shapes[side]+32));put(shapes[side]+32,0x21c,present?(unsigned)(uintptr_t)registered[side]:0);
  }
  memcpy(t+0x50,e+0x10,16);memcpy(t+0x60,e+0x20,16);memcpy(t+0x3c,e+0x40,4);memcpy(t+0x30,e+8,8);
  unsigned cursor=0;
  #define SNAP(buffer) memcpy(contexts_before+cursor,buffer,sizeof(buffer));cursor+=(unsigned)sizeof(buffer)
  SNAP(regs);SNAP(objects);SNAP(other_body);SNAP(shapes);SNAP(contact);SNAP(event);
  #undef SNAP
  memset(packets,0,sizeof(packets));packet_count=packet_error=0;
  for(unsigned j=0;j<11;j++)if(get(r,48)&(1u<<j)){put(v,0x37098+j*4,(unsigned)(uintptr_t)notification);put(v,0x370c4+j*4,0x24680000+j);}
  for(unsigned j=0;j<6;j++)memcpy(image+pages[j]-0x400000,snapshot+pages[j]-0x400000,4096);
  put(image,0xe4fcc0-0x400000,get(r,0));
  __try{
   if(selector<2)((void (__fastcall *)(void *,void *,void *,unsigned,unsigned,unsigned))(image+0x9ef360-0x400000))(v,NULL,e,selector,get(r,12),0);
   else ((void (__stdcall *)(void *,unsigned,unsigned))(image+0xa726b0-0x400000))(t,0x13579bdf,get(r,12));
  }__except(fault(GetExceptionInformation())){error=12;}
  cursor=0;
  #define CHECK(buffer) if(memcmp(contexts_before+cursor,buffer,sizeof(buffer)))error=13;cursor+=(unsigned)sizeof(buffer)
  CHECK(regs);CHECK(objects);CHECK(other_body);CHECK(shapes);CHECK(contact);CHECK(event);
  #undef CHECK
  if(memcmp(component,component_before,sizeof(component))||memcmp(wrapper,wrapper_before,sizeof(wrapper))||packet_error)error=13;
  for(unsigned j=0;j<11;j++){
   unsigned callback=(get(r,48)&(1u<<j))?(unsigned)(uintptr_t)notification:0,user=(get(r,48)&(1u<<j))?0x24680000+j:0;
   if(get(v,0x37098+j*4)!=callback||get(v,0x370c4+j*4)!=user)error=13;
   put(v,0x37098+j*4,0);put(v,0x370c4+j*4,0);
  }
  if(get(v,0xdb0)!=(unsigned)(uintptr_t)w||get(v,MUTABLE+0x214)!=(unsigned)(uintptr_t)c||get(p,0x424)!=(unsigned)(uintptr_t)w)error=13;
  put(v,0xdb0,0);put(v,MUTABLE+0x214,0);put(p,0x424,0);
  if(!canary(owner,TAIL)||!canary(physical,0x430)||memcmp(p,r+PREFIX+CAR,PHYSICAL))error=13;
  for(unsigned j=CAR;j<TAIL;j++)if(v[j])error=13;
  for(unsigned j=PHYSICAL;j<0x430;j++)if(p[j])error=13;
  unsigned char *out=outputs+8+i*RESULT;put(out,0,v[0xba5]);put(out,4,get(image,0xe4fcc0-0x400000));put(out,8,packet_count);memcpy(out+12,packets,sizeof(packets));memcpy(out+12+sizeof(packets),v,CAR);memcpy(out+12+sizeof(packets)+CAR,p,PHYSICAL);
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,prior[i],&ignored))error=15;}
 if(memcmp(image,snapshot,image_size))error=16;
 if(!error){HANDLE out=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(out==INVALID_HANDLE_VALUE)error=17;else{DWORD written;if(!WriteFile(out,outputs,8+count*RESULT,&written,NULL)||written!=8+count*RESULT)error=18;if(!CloseHandle(out))error=19;}}
 printf("body_impact_cases=%u code_unchanged=%u globals_restored=%u owned_contexts_checked=1 code_replacements=0 error=%u\n",count,error!=16,error!=15&&error!=16,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return error;
}
