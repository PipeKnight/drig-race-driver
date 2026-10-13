/* Whole original A727A0/A726B0/A01910/9EF360 on solved source records.
   Explicit single car, empty notifications, transient prefix and one owned
   geometry-pair carrier. No world solver/resource factory is executed here. */
#define GRID_COLLISION_DAMAGE_HELPERS
#include "probe_collision_damage.c"
#define SLOTS 20
#define PREFIX 16
#define RECORD (PREFIX+CAR+PHYSICAL+SLOTS*0x80)
#define RESULT (12+CAR+PHYSICAL+SLOTS*0x80)
#define OWNER_BYTES 0x37100

static int pool_admitted(const unsigned char *r){
 const unsigned char *v=r+PREFIX,*p=v+CAR,*slots=p+PHYSICAL;
 if(get(r,8)>SLOTS||get(r,12)>get(r,8)||get(r,12)>4||!range(r,4,.000001f,.1f)||v[0x308]||v[0xb13]>1||v[0xb14]>1)return 0;
 if(!range(v,0x15a0,0.f,1.e9f)||!range(v,0x15a4,0.f,1000.f)||!range(v,0x2e8,-1000.f,1000.f)||!range(v,0x3d0,-1000.f,1000.f))return 0;
 for(unsigned j=0;j<4;j++)if(!range(p,0x220+j*4,-10000.f,10000.f))return 0;
 unsigned char owner_test[SIZE];memset(owner_test,0,32);put(owner_test,0,get(r,0));
 memcpy(owner_test+16,p+0x200,16);memcpy(owner_test+32,v,CAR+PHYSICAL);
 if(!admitted(owner_test))return 0;
 for(unsigned i=0;i<get(r,8);i++){
  const unsigned char *c=slots+i*0x80;unsigned a=get(c,0x28),b=get(c,0x2c);
  if(!((a==1&&b>=2&&b<=513)||(b==1&&a>=2&&a<=513)))return 0;
  memcpy(owner_test+16,c+0x50,16);put(owner_test,8,get(c,0x3c)==0x7f7fffff?0:get(c,0x3c));
  if(!admitted(owner_test))return 0;
  for(unsigned j=0;j<4;j++)if(!range(c,0x60+j*4,-1000.f,1000.f))return 0;
 }
 return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 unsigned h[2];if(fread(h,4,2,file)!=2||h[0]!=0x31305742||!h[1]||h[1]>64){fclose(file);return 5;}
 unsigned count=h[1],error=0;unsigned char *inputs=malloc(count*RECORD),*outputs=malloc(8+count*RESULT);
 if(!inputs||!outputs){fclose(file);free(inputs);free(outputs);return 6;}
 int valid=fread(inputs,1,count*RECORD,file)==count*RECORD&&fgetc(file)==EOF;fclose(file);
 if(!valid){free(inputs);free(outputs);return 7;}
 for(unsigned i=0;i<count;i++)if(!pool_admitted(inputs+i*RECORD)){free(inputs);free(outputs);return 8;}
 memcpy(outputs,h,8);unsigned char *image=map_reference(argv[1]);if(!image){free(inputs);free(outputs);return 9;}reference=(uintptr_t)image;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *snapshot=malloc(image_size);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return 10;}memcpy(snapshot,image,image_size);
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000,0xe85000,0xe4f000};DWORD prior[6],ignored;unsigned protected=0;
 for(unsigned i=0;i<6;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,prior+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=inputs+i*RECORD;
  __declspec(align(16)) unsigned char owner[32+OWNER_BYTES+32],physical[32+0x430+32],component[32+0x100+32],wrapper[32+0x10+32];
  __declspec(align(16)) unsigned char world_owned[32+0x810+32],producer_owned[32+0xc40+32],world_wrapper_owned[32+0x10+32],pair_owned[32+0x50+32];
  __declspec(align(16)) unsigned char registration_owned[32+0x20+32],shapes[2][32+0x220+32],contacts_owned[32+SLOTS*0x80+32];
  unsigned char world_before[sizeof(world_owned)],producer_before[sizeof(producer_owned)],world_wrapper_before[sizeof(world_wrapper_owned)],pair_before[sizeof(pair_owned)],registration_before[sizeof(registration_owned)],shapes_before[sizeof(shapes)],contacts_before[sizeof(contacts_owned)],component_before[sizeof(component)],wrapper_before[sizeof(wrapper)];
  unsigned char *v=owner+32,*p=physical+32,*c=component+32,*w=wrapper+32,*world=world_owned+32,*producer=producer_owned+32,*ww=world_wrapper_owned+32,*pair=pair_owned+32,*reg=registration_owned+32,*pool=contacts_owned+32;
  memset(owner,0xcd,sizeof(owner));memset(v,0,OWNER_BYTES);memcpy(v,r+PREFIX,CAR);
  memset(physical,0xcd,sizeof(physical));memset(p,0,0x430);memcpy(p,r+PREFIX+CAR,PHYSICAL);
  #define ZERO_OWNED(buffer,bytes) memset(buffer,0xcd,sizeof(buffer));memset(buffer+32,0,bytes)
  ZERO_OWNED(component,0x100);ZERO_OWNED(wrapper,0x10);ZERO_OWNED(world_owned,0x810);ZERO_OWNED(producer_owned,0xc40);ZERO_OWNED(world_wrapper_owned,0x10);ZERO_OWNED(pair_owned,0x50);ZERO_OWNED(registration_owned,0x20);
  #undef ZERO_OWNED
  memset(shapes,0xcd,sizeof(shapes));memset(shapes[0]+32,0,0x220);memset(shapes[1]+32,0,0x220);
  memset(contacts_owned,0xcd,sizeof(contacts_owned));memcpy(pool,r+PREFIX+CAR+PHYSICAL,SLOTS*0x80);
  put(w,8,(unsigned)(uintptr_t)p);put(p,0x424,(unsigned)(uintptr_t)w);put(v,0xdb0,(unsigned)(uintptr_t)w);put(c,0x24,(unsigned)(uintptr_t)v);put(v,MUTABLE+0x214,(unsigned)(uintptr_t)c);
  put(shapes[0]+32,0xd8,(unsigned)(uintptr_t)p);put(shapes[0]+32,0x21c,(unsigned)(uintptr_t)reg);
  put(reg,8,(unsigned)(uintptr_t)(shapes[0]+32));put(reg,0xc,(unsigned)(uintptr_t)(image+0xa01910-0x400000));put(reg,0x10,(unsigned)(uintptr_t)v);
  for(unsigned j=0;j<get(r,8);j++)for(unsigned k=0;k<2;k++){
   unsigned offset=0x28+k*4,token=get(pool+j*0x80,offset);
   put(pool+j*0x80,offset,(unsigned)(uintptr_t)(shapes[token==1?0:1]+32));
  }
  put(ww,4,(unsigned)(uintptr_t)world);put(world,0x18,(unsigned)(uintptr_t)producer);put(world,0x7c8,get(r,4));
  put(producer,0x91c,(unsigned)(uintptr_t)pool);put(producer,0x924,get(r,12));
  put(producer,0x930,get(r,8)>get(r,12)?(unsigned)(uintptr_t)pair:0);
  put(pair,0x20,(unsigned)(uintptr_t)(pool+get(r,12)*0x80));put(pair,0x24,get(r,8)-get(r,12));
  memcpy(world_before,world_owned,sizeof(world_owned));memcpy(producer_before,producer_owned,sizeof(producer_owned));memcpy(world_wrapper_before,world_wrapper_owned,sizeof(world_wrapper_owned));memcpy(pair_before,pair_owned,sizeof(pair_owned));memcpy(registration_before,registration_owned,sizeof(registration_owned));memcpy(shapes_before,shapes,sizeof(shapes));memcpy(contacts_before,contacts_owned,sizeof(contacts_owned));memcpy(component_before,component,sizeof(component));memcpy(wrapper_before,wrapper,sizeof(wrapper));
  for(unsigned j=0;j<6;j++)memcpy(image+pages[j]-0x400000,snapshot+pages[j]-0x400000,4096);put(image,0xe4fcc0-0x400000,get(r,0));
  __try {((void (__fastcall *)(void *,void *))(image+0xa727a0-0x400000))(ww,NULL);}__except(fault(GetExceptionInformation())){error=12;}
  put(producer_before+32,0x924,0);
  if(memcmp(world_before,world_owned,sizeof(world_owned))||memcmp(producer_before,producer_owned,sizeof(producer_owned))||memcmp(world_wrapper_before,world_wrapper_owned,sizeof(world_wrapper_owned))||memcmp(pair_before,pair_owned,sizeof(pair_owned))||memcmp(registration_before,registration_owned,sizeof(registration_owned))||memcmp(shapes_before,shapes,sizeof(shapes))||memcmp(contacts_before,contacts_owned,sizeof(contacts_owned))||memcmp(component_before,component,sizeof(component))||memcmp(wrapper_before,wrapper,sizeof(wrapper)))error=13;
  if(get(v,0xdb0)!=(unsigned)(uintptr_t)w||get(v,MUTABLE+0x214)!=(unsigned)(uintptr_t)c||get(p,0x424)!=(unsigned)(uintptr_t)w)error=13;
  put(v,0xdb0,0);put(v,MUTABLE+0x214,0);put(p,0x424,0);
  if(!canary(owner,OWNER_BYTES)||!canary(physical,0x430)||memcmp(p,r+PREFIX+CAR,PHYSICAL))error=13;
  for(unsigned j=CAR;j<OWNER_BYTES;j++)if(v[j])error=13;
  for(unsigned j=PHYSICAL;j<0x430;j++)if(p[j])error=13;
  unsigned char *out=outputs+8+i*RESULT;put(out,0,get(image,0xe4fcc0-0x400000));put(out,4,get(producer,0x924));put(out,8,get(pair,0x24));memcpy(out+12,v,CAR);memcpy(out+12+CAR,p,PHYSICAL);memcpy(out+12+CAR+PHYSICAL,r+PREFIX+CAR+PHYSICAL,SLOTS*0x80);
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,prior[i],&ignored))error=15;}
 if(memcmp(image,snapshot,image_size))error=16;
 if(!error){HANDLE out=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(out==INVALID_HANDLE_VALUE)error=17;else{DWORD written;if(!WriteFile(out,outputs,8+count*RESULT,&written,NULL)||written!=8+count*RESULT)error=18;if(!CloseHandle(out))error=19;}}
 printf("body_pool_cases=%u globals_restored=%u complete_image_unchanged=%u owned_contexts_checked=1 original_wrapper=1 code_replacements=0 error=%u\n",count,error!=15&&error!=16,error!=16,error);
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);free(inputs);free(outputs);return error;
}
