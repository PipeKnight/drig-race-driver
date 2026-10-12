/* Whole unchanged B44E80 source collision-mesh construction on owned storage.
   No allocator, shape registration, collision detection or game process attach. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <float.h>
#include <math.h>
#include "isolated_reference.h"
static unsigned get(const void *p,unsigned o){unsigned v;memcpy(&v,(const unsigned char *)p+o,4);return v;}
static void put(void *p,unsigned o,unsigned v){memcpy((unsigned char *)p+o,&v,4);}
static unsigned align16(unsigned n){return (n+15u)&~15u;}
static int canonical(unsigned char *body,unsigned o,unsigned length){
 unsigned pointer=get(body,o),base=(unsigned)(uintptr_t)body;
 if(pointer<base||pointer-base>=length||((pointer-base)&3u))return 0;
 put(body,o,pointer-base);return 1;
}
#ifdef GRID_MESH_ATTACH
static uintptr_t reference_base;
static LONG attach_fault(EXCEPTION_POINTERS *e){fprintf(stderr,"attach_fault=%08lx original_eip=%08lx address=%08lx\n",e->ExceptionRecord->ExceptionCode,(DWORD)(e->ContextRecord->Eip-reference_base+0x400000),(DWORD)e->ExceptionRecord->ExceptionInformation[1]);return EXCEPTION_EXECUTE_HANDLER;}
static float number(const void *p,unsigned o){float v;memcpy(&v,(const char *)p+o,4);return v;}
static int physical_admitted(const unsigned char *s){
 if(get(s,0x60)!=1||get(s,0x64)||get(s,0x134))return 0;
 for(unsigned i=0x130;i<0x134;i++)if(s[i]>1)return 0;
 for(unsigned i=0x68;i<0x90;i+=4)if(get(s,i))return 0;
 for(unsigned i=0;i<0x160;i+=4)if(i!=0x60&&i!=0x64&&i!=0x130&&i!=0x134&&(!isfinite(number(s,i))||((i<0xa0||i>0xb0)&&i!=0xec&&fabsf(number(s,i))>1000000.f)))return 0;
 if(number(s,0)<=0||!isfinite(1.f/number(s,0)))return 0;
 for(unsigned r=0;r<4;r++)for(unsigned c=0;c<4;c++){float v=number(s,0x10+r*16+c*4);if((r==c&&(v<=0||(r==3&&v!=1)))||(r!=c&&get(s,0x10+r*16+c*4)))return 0;}
 float cd=number(s,0x24)*number(s,0x38),det=cd*number(s,0x10);return isnormal(det)&&det>0;
}
#endif
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 #ifdef GRID_MESH_ATTACH
 reference_base=(uintptr_t)image;
#endif
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 17;}
 memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) float points[80][4];unsigned triangles[110][3];
#ifdef GRID_SOURCE_COLLISION_MESH
 float source_parameters[6];
#ifdef GRID_MESH_SHAPE
 unsigned shape_parameters[4];float shape_scalar;
 #ifdef GRID_MESH_BASE
 unsigned base_descriptor[33],base_controls[5];
#ifdef GRID_MESH_ATTACH
 __declspec(align(16)) unsigned char physical_settings[0x160],physical_owned[32+0x430+32],registry_owned[32+0x100+32],root_owned[32+0xa80+32],cell_owned[32+16+32];
 unsigned char *physical_body=physical_owned+32,*registry=registry_owned+32,*root=root_owned+32,*cells=cell_owned+32;
#endif
 #define SHAPE_STORAGE 0x500
#else
 #define SHAPE_STORAGE 0x250
#endif
 __declspec(align(16)) unsigned char shape_owned[32+SHAPE_STORAGE+32];unsigned char *shape=shape_owned+32;
 unsigned char mesh_before_shape[16000];
#endif
 __declspec(align(16)) float raw_points[80][4];unsigned char raw_before[sizeof(raw_points)];
 __declspec(align(16)) unsigned char car_owned[32+0x21000+32],car_before[0x21000];unsigned char *car=car_owned+32;
#endif
 __declspec(align(16)) unsigned char owned[32+16000+32];unsigned char *body=owned+32;
 memset(owned,0xcd,sizeof(owned));
 unsigned error=0,protected=0,tag=0,count=0,faces=0,fill=0,length=0,restored=1;
 #ifdef GRID_MESH_ATTACH
 const unsigned data_pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};
#else
 const unsigned data_pages[]={0xe58000};
#endif
 DWORD data_protections[sizeof(data_pages)/sizeof(data_pages[0])];
 unsigned char saved[sizeof(data_pages)/sizeof(data_pages[0])*4096];FILE *input=NULL;
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
 int valid=fread(&tag,4,1,input)==1&&fread(&count,4,1,input)==1&&fread(&faces,4,1,input)==1&&fread(&fill,4,1,input)==1;
 if(!valid||tag!=
#ifdef GRID_MESH_ATTACH
 0x35304d43
#elif defined(GRID_MESH_BASE)
 0x34304d43
#elif defined(GRID_MESH_SHAPE)
 0x33304d43
#elif defined(GRID_SOURCE_COLLISION_MESH)
 0x32304d43
#else
 0x31304d43
#endif
 ||count<4||count>80||faces<4||faces>110||(fill!=0&&fill!=0xcd)){error=6;goto done;}
#ifdef GRID_SOURCE_COLLISION_MESH
 if(fread(source_parameters,4,6,input)!=6){error=6;goto done;}
 for(unsigned i=0;i<6;i++)if(!isfinite(source_parameters[i])||fabsf(source_parameters[i])>10000.f)error=22;
 if(source_parameters[3]!=0.f||source_parameters[4]>source_parameters[5])error=22;
 if(error)goto done;
#ifdef GRID_MESH_SHAPE
#ifdef GRID_MESH_BASE
 if(fread(base_descriptor,4,33,input)!=33||fread(base_controls,4,5,input)!=5){error=6;goto done;}
 if(base_descriptor[0]||base_descriptor[0x5c/4]||base_controls[0]>1000000||base_controls[1]>1||base_controls[4]>1000000){error=28;goto done;}
 for(unsigned i=0x10;i<0x50;i+=4){float v;memcpy(&v,(char *)base_descriptor+i,4);if(!isfinite(v)||fabsf(v)>10000.f)error=28;}
 for(unsigned i=0;i<3;i++){float v;unsigned o=0x58+i*8;memcpy(&v,(char *)base_descriptor+o,4);if(!isfinite(v)||fabsf(v)>1e6f)error=28;}
 const unsigned flags[]={0x50,0x51,0x52,0x53,0x54,0x55,0x71,0x79,0x7a};
 for(unsigned i=0;i<sizeof(flags)/sizeof(flags[0]);i++)if(((unsigned char *)base_descriptor)[flags[i]]>1)error=28;
 if(error)goto done;
#ifdef GRID_MESH_ATTACH
 if(fread(physical_settings,1,0x160,input)!=0x160||!physical_admitted(physical_settings)||((unsigned char *)base_descriptor)[0x80]>31||number(base_descriptor,0x68)<0.f){error=30;goto done;}
 memset(physical_owned,0xcd,sizeof(physical_owned));memset(registry_owned,0xcd,sizeof(registry_owned));memset(root_owned,0xcd,sizeof(root_owned));memset(cell_owned,0xcd,sizeof(cell_owned));
 memset(physical_body,(int)fill,0x430);memset(registry,0,0x100);memset(cells,0,16);
#endif
 shape_parameters[0]=base_descriptor[1];shape_parameters[1]=base_descriptor[0x68/4];shape_parameters[2]=((unsigned char *)base_descriptor)[0x80];shape_parameters[3]=((unsigned char *)base_descriptor)[0x54];
#else
 if(fread(shape_parameters,4,4,input)!=4){error=6;goto done;}
#endif
 memcpy(&shape_scalar,shape_parameters+1,4);
 if(!shape_parameters[0]||shape_parameters[0]>16000||!isfinite(shape_scalar)||fabsf(shape_scalar)>1e6f||shape_parameters[2]>255||shape_parameters[3]>1){error=25;goto done;}
#endif
#endif
 valid=fread(points,16,count,input)==count&&fread(triangles,12,faces,input)==faces&&fgetc(input)==EOF;
 fclose(input);input=NULL;if(!valid){error=6;goto done;}
 unsigned edges=0;
 for(unsigned i=0;i<count;i++)for(unsigned j=0;j<4;j++)if(!isfinite(points[i][j])||fabsf(points[i][j])>100000.f||(j==3&&points[i][j]!=1.f))error=7;
#ifdef GRID_SOURCE_COLLISION_MESH
 if(error)goto done;
 memcpy(raw_points,points,count*16);memcpy(raw_before,raw_points,count*16);
 memset(car_owned,0xcd,sizeof(car_owned));memset(car,0,0x21000);
 memcpy(car+0x1630,source_parameters,16);memcpy(car+0xed68,source_parameters+4,4);memcpy(car+0xed94,source_parameters+5,4);memcpy(car_before,car,0x21000);
 __try{((void (__fastcall *)(void *,void *,void *,void *,int))(image+0x9b8b70-0x400000))(car,NULL,points,raw_points,(int)count);}
 __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"source_adjustment_fault=%08lx\n",GetExceptionCode());error=23;}
 if(memcmp(car,car_before,0x21000)||memcmp(raw_points,raw_before,count*16))error=24;
 for(unsigned i=0;i<32;i++)if(car_owned[i]!=0xcd||car_owned[32+0x21000+i]!=0xcd)error=24;
 for(unsigned i=0;i<count;i++)for(unsigned j=0;j<4;j++)if(!isfinite(points[i][j])||fabsf(points[i][j])>100000.f||(j==3&&points[i][j]!=1.f))error=7;
 if(error)goto done;
#endif
 for(unsigned i=0;i<faces;i++){
  unsigned a=triangles[i][0],b=triangles[i][1],c=triangles[i][2];
  if(a>=count||b>=count||c>=count||a==b||b==c||a==c){error=8;break;}
  float ab[3],ac[3],n[3];for(unsigned j=0;j<3;j++){ab[j]=points[b][j]-points[a][j];ac[j]=points[c][j]-points[a][j];}
  n[0]=ab[1]*ac[2]-ab[2]*ac[1];n[1]=ac[0]*ab[2]-ac[2]*ab[0];n[2]=ac[1]*ab[0]-ac[0]*ab[1];
  float squared=(n[0]*n[0]+n[1]*n[1])+n[2]*n[2];if(!isfinite(squared)||squared<1e-20f||squared>1e20f)error=8;
 }
 if(error)goto done;
 for(unsigned i=0;i<count;i++){
  unsigned char seen[80]={0};
  for(unsigned j=0;j<faces;j++)if(triangles[j][0]==i||triangles[j][1]==i||triangles[j][2]==i)
   for(unsigned k=0;k<3;k++)if(triangles[j][k]!=i)seen[triangles[j][k]]=1;
  for(unsigned j=0;j<count;j++)edges+=seen[j];
 }
 if(edges>10*count){error=9;goto done;}
 unsigned settings[12]={0x60,count*32,count*16,align16(count*40),align16(faces*12),faces*32,count,faces,faces*3,count*10,(unsigned)(uintptr_t)points,(unsigned)(uintptr_t)triangles};
 unsigned char point_before[sizeof(points)],triangle_before[sizeof(triangles)],settings_before[sizeof(settings)];
 memcpy(point_before,points,count*16);memcpy(triangle_before,triangles,faces*12);memcpy(settings_before,settings,sizeof(settings));
 length=0x60+settings[1]+settings[2]+settings[3]+settings[4]+settings[5];
 if(length>16000){error=9;goto done;}memset(body,(int)fill,length);
 for(unsigned i=0;i<sizeof(data_pages)/sizeof(data_pages[0]);i++){
  unsigned char *page=image+data_pages[i]-0x400000;memcpy(saved+i*4096,page,4096);
  if(!VirtualProtect(page,4096,PAGE_READWRITE,data_protections+i)){error=10;goto done;}++protected;
 }
 __try{((void (__cdecl *)(void *,void *))(image+0xb44e80-0x400000))(body,settings);}
 __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"mesh_fault=%08lx\n",GetExceptionCode());error=11;}
 if(memcmp(point_before,points,count*16)||memcmp(triangle_before,triangles,faces*12)||memcmp(settings_before,settings,sizeof(settings)))error=21;
#ifdef GRID_MESH_SHAPE
 if(!error){
  __declspec(align(16)) unsigned descriptor[33]={0};descriptor[0]=(unsigned)(uintptr_t)body;descriptor[1]=shape_parameters[0];descriptor[0x1a]=shape_parameters[1];descriptor[0x20]=shape_parameters[2];descriptor[0x15]=shape_parameters[3];
#ifdef GRID_MESH_BASE
  memcpy(descriptor,base_descriptor,sizeof(descriptor));descriptor[0]=(unsigned)(uintptr_t)body;
#endif
  unsigned descriptor_before[33];memcpy(descriptor_before,descriptor,sizeof(descriptor));
  memcpy(mesh_before_shape,body,length);memset(shape_owned,0xcd,sizeof(shape_owned));memset(shape,(int)fill,SHAPE_STORAGE);
#ifdef GRID_MESH_BASE
  unsigned char *shape_world=
#ifdef GRID_MESH_ATTACH
 root
#else
 shape+0x2e0
#endif
 ,*wrapper=shape+0x3e0,*material_manager=shape+0x400,*materials=shape+0x420;
  memset(shape_world,0,
#ifdef GRID_MESH_ATTACH
 0xa80
#else
 0x100
#endif
 );memset(material_manager,0,0x20);memset(materials,0,0x40);
  put(shape_world,0x14,(unsigned)(uintptr_t)material_manager);put(shape_world,0x80,base_controls[0]);
  put(material_manager,4,(unsigned)(uintptr_t)materials);put(material_manager,8,2);put(material_manager,0xc,0x20);material_manager[0x10]=(unsigned char)base_controls[1];
  put(materials,0,base_controls[2]);put(materials,0x20,base_controls[3]);descriptor[0x5c/4]=(unsigned)(uintptr_t)shape_world;
#ifdef GRID_MESH_ATTACH
  put(root,0x940,0x447a0000);put(root,0x944,0x447a0000);put(root,0x948,1);put(root,0x94c,1);
  put(root,0x950,(unsigned)(uintptr_t)cells);put(root,0x954,(unsigned)(uintptr_t)(cells+8));put(registry,0x18,(unsigned)(uintptr_t)root);put(physical_settings,0x64,(unsigned)(uintptr_t)registry);
#endif
#endif
  __try{
#ifdef GRID_MESH_BASE
   ((void (__fastcall *)(void *,void *,void *))(image+0xb460f0-0x400000))(shape,NULL,descriptor);
   ((void (__fastcall *)(void *,void *,void *,unsigned))(image+0xb23d70-0x400000))(wrapper,NULL,shape,base_controls[4]);
#ifdef GRID_MESH_ATTACH
   __try{((void (__fastcall *)(void *,void *,void *))(image+0xb11f40-0x400000))(physical_body,NULL,physical_settings);}
   __except(attach_fault(GetExceptionInformation())){error=31;}
   if(!error){__try{((void (__fastcall *)(void *,void *,void *,unsigned))(image+0xb142d0-0x400000))(physical_body,NULL,shape,1);}
   __except(attach_fault(GetExceptionInformation())){error=32;}}
#endif
#else
   ((void (__fastcall *)(void *,void *,void *))(image+0xb444d0-0x400000))(shape,NULL,descriptor);
#endif
  }
  __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"mesh_shape_fault=%08lx\n",GetExceptionCode());error=26;}
#ifndef GRID_MESH_BASE
  if(memcmp(descriptor,descriptor_before,sizeof(descriptor)))error=27;
#else
  (void)descriptor_before;
  if(get(shape_world,0x80)!=base_controls[0]+1||get(shape,0)!=(unsigned)(uintptr_t)(image+0xd3e7e0-0x400000)||get(wrapper,0)!=(unsigned)(uintptr_t)(image+0xd3d4c4-0x400000)||get(shape,0x21c)!=(unsigned)(uintptr_t)wrapper||get(shape,0xcc)!=(unsigned)(uintptr_t)shape_world||get(wrapper,8)!=(unsigned)(uintptr_t)shape||get(wrapper,0x14)!=(unsigned)(uintptr_t)shape)error=29;
  put(shape,0,0x7000);put(wrapper,0,0x7001);put(wrapper,8,0x1000);put(wrapper,0x14,0x1000);put(shape,0x21c,0x13e0);put(shape,0xcc,0x12e0);
  unsigned selected=base_controls[1]?0x1420:0;
  if(base_controls[1]&&descriptor[0x7c/4]==base_controls[3]&&descriptor[0x7c/4]!=base_controls[2])selected+=0x20;
  unsigned expected_material=selected?(unsigned)(uintptr_t)(shape+selected-0x1000):0;
  if(get(shape,0xc8)!=expected_material)error=29;else put(shape,0xc8,selected);
  const unsigned pointers[]={0xb0,0xb4,0xb8,0xbc,0x170,0x174,0x178,0x17c,0x180,0x1a4,0x1b8,0x1cc,0x1e0,0x1f4,0x208};
  for(unsigned i=0;i<sizeof(pointers)/sizeof(pointers[0]);i++){
   unsigned value=get(shape,pointers[i]),base=(unsigned)(uintptr_t)shape;
   if(value<base||value>=base+0x250)error=29;else put(shape,pointers[i],0x1000+value-base);
  }
#ifdef GRID_MESH_ATTACH
  if(!error){
   if(get(physical_body,0x328)!=(unsigned)(uintptr_t)shape||get(shape,0xd8)!=(unsigned)(uintptr_t)physical_body||get(shape,0xe0)||get(shape,0x198)!=1||get(cells,12)!=(unsigned)(uintptr_t)shape){error=33;}
   put(physical_body,0x328,0x1000);put(shape,0xd8,0xa000);put(cells,12,0x1000);
   const unsigned body_pointers[]={8,0xc,0x18,0x80,0x84,0x88,0x8c,0x330,0x334,0x338,0x33c,0x340};
   for(unsigned i=0;i<sizeof(body_pointers)/sizeof(body_pointers[0]);i++){unsigned value=get(physical_body,body_pointers[i]),base=(unsigned)(uintptr_t)physical_body;if(value<base||value>=base+0x430)error=33;else put(physical_body,body_pointers[i],value-base+1);}
   if(get(physical_body,0x32c)!=(unsigned)(uintptr_t)registry||get(physical_settings,0x64)!=(unsigned)(uintptr_t)registry)error=33;else {put(physical_body,0x32c,0x9000);put(physical_settings,0x64,0);}
   for(unsigned i=0;i<0x100;i+=4){unsigned value=get(registry,i);if(i==0x18){if(value!=(unsigned)(uintptr_t)root)error=33;else put(registry,i,0x12e0);}else if(i!=0x60&&value){if(value!=(unsigned)(uintptr_t)physical_body)error=33;else put(registry,i,0xa000);}}
   const unsigned endpoint_pointers[]={0x19c,0x1a0,0x1b0,0x1b4,0x1c4,0x1c8,0x1d8,0x1dc,0x1ec,0x1f0,0x200,0x204};
   for(unsigned i=0;i<sizeof(endpoint_pointers)/sizeof(endpoint_pointers[0]);i++){unsigned value=get(shape,endpoint_pointers[i]),base=(unsigned)(uintptr_t)shape;if(value){if(value<base||value>=base+0x250)error=33;else put(shape,endpoint_pointers[i],0x1000+value-base);}}
   for(unsigned i=0;i<3;i++){unsigned value=get(root,0x900+i*4),expected=(unsigned)(uintptr_t)(shape+0x19c+i*40);if(value!=expected)error=33;else put(root,0x900+i*4,0x119c+i*40);}
   if(get(root,0x950)!=(unsigned)(uintptr_t)cells||get(root,0x954)!=(unsigned)(uintptr_t)(cells+8))error=33;else {put(root,0x950,0xb000);put(root,0x954,0xb008);}
   for(unsigned i=0;i<32;i++)if(physical_owned[i]!=0xcd||physical_owned[32+0x430+i]!=0xcd||registry_owned[i]!=0xcd||registry_owned[32+0x100+i]!=0xcd||root_owned[i]!=0xcd||root_owned[32+0xa80+i]!=0xcd||cell_owned[i]!=0xcd||cell_owned[32+16+i]!=0xcd)error=34;
   for(unsigned i=0x90;i<0x2f0;i+=4)if(!isfinite(number(physical_body,i)))error=35;
  }
#endif
  put(shape_world,0x14,0x1400);put(material_manager,4,0x1420);descriptor[0]=0x1500;descriptor[0x5c/4]=0x12e0;memcpy(shape+0x250,descriptor,sizeof(descriptor));
#ifdef GRID_MESH_ATTACH
  memcpy(shape+0x2e0,root,0x100);
#endif
#endif
  for(unsigned i=0;i<32;i++)if(shape_owned[i]!=0xcd||shape_owned[32+SHAPE_STORAGE+i]!=0xcd)error=27;
  unsigned planes=0x60+count*48;
  for(unsigned i=0;i<length;i++)if(!(i>=planes&&i<planes+faces*32&&((i-planes)%32)<16)&&body[i]!=mesh_before_shape[i])error=27;
  if(get(shape,0x240)!=(unsigned)(uintptr_t)body)error=27;else put(shape,0x240,
#ifdef GRID_MESH_BASE
 0x1500
#else
 0x250
#endif
 );
 }
#endif
 if(!error){
  unsigned vertex=0x60,aux=vertex+count*32,face=aux+count*16,neighbor=face+faces*32,incident=neighbor+align16(count*40);
  if(get(body,0)!=count||get(body,4)!=faces||get(body,0x54)!=(unsigned)(uintptr_t)(body+vertex)||get(body,0x58)!=(unsigned)(uintptr_t)(body+face))error=12;
  if(!canonical(body,0x54,length)||!canonical(body,0x58,length))error=12;
  unsigned edge_cursor=0,incident_cursor=0;
  for(unsigned i=0;i<count&&!error;i++){
   unsigned v=vertex+i*32,a=aux+i*16,n=get(body,a),t=get(body,v+0x14);
   if(n>count||t>faces||get(body,v+0x10)!=(unsigned)(uintptr_t)(body+a)||get(body,a+4)!=(unsigned)(uintptr_t)(body+neighbor+edge_cursor*4)||get(body,v+0x18)!=(unsigned)(uintptr_t)(body+incident+incident_cursor*4)){error=12;break;}
   if(!canonical(body,v+0x10,length)||!canonical(body,a+4,length)||!canonical(body,v+0x18,length))error=12;
   for(unsigned j=0;j<n;j++)if(!canonical(body,neighbor+(edge_cursor+j)*4,length))error=12;
   for(unsigned j=0;j<t;j++)if(!canonical(body,incident+(incident_cursor+j)*4,length))error=12;
   edge_cursor+=n;incident_cursor+=t;
  }
  if(edge_cursor!=edges||incident_cursor!=3*faces)error=12;
  for(unsigned i=0;i<faces;i++)for(unsigned j=0;j<4;j++){float value;memcpy(&value,body+face+i*32+j*4,4);if(!isfinite(value))error=13;}
 }
 for(unsigned i=0;i<32;i++)if(owned[i]!=0xcd||owned[32+length+i]!=0xcd)error=14;
done:
 if(input)fclose(input);
 for(unsigned i=0;i<protected;i++){DWORD ignored;unsigned char *restore_page=image+data_pages[i]-0x400000;memcpy(restore_page,saved+i*4096,4096);if(!VirtualProtect(restore_page,4096,data_protections[i],&ignored)||memcmp(restore_page,saved+i*4096,4096)){error=15;restored=0;}}
 unsigned code_unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))code_unchanged=0;
 if(!code_unchanged)error=18;
#ifdef GRID_MESH_ATTACH
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData)){restored=0;error=15;}
#endif
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=19;else{DWORD n=0;
#ifdef GRID_MESH_SHAPE
 if(!WriteFile(output,shape,SHAPE_STORAGE,&n,NULL)||n!=SHAPE_STORAGE)error=20;
#endif
 if(!WriteFile(output,body,length,&n,NULL)||n!=length)error=20;
#ifdef GRID_MESH_ATTACH
 const void *parts[]={physical_body,physical_settings,registry,root,cells};const DWORD lengths[]={0x430,0x160,0x100,0xa80,16};
 for(unsigned i=0;i<5;i++){if(!WriteFile(output,parts[i],lengths[i],&n,NULL)||n!=lengths[i])error=20;}
#endif
 CloseHandle(output);}}
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);
 printf("mesh_bytes=%u code_unchanged=%u code_replacements=0 globals_restored=%u error=%u\n",length,code_unchanged,restored,error);return (int)error;
}
