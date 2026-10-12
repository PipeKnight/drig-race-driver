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
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 unsigned char *image=map_reference(argv[1]);if(!image)return 4;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot){VirtualFree(image,0,MEM_RELEASE);return 17;}
 memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 __declspec(align(16)) float points[80][4];unsigned triangles[110][3];
 __declspec(align(16)) unsigned char owned[32+16000+32];unsigned char *body=owned+32;
 memset(owned,0xcd,sizeof(owned));
 unsigned error=0,protected=0,tag=0,count=0,faces=0,fill=0,length=0,restored=1;DWORD protection=0;
 unsigned char saved[4096];FILE *input=NULL;
 if(_wfopen_s(&input,argv[2],L"rb")||!input){error=5;goto done;}
 int valid=fread(&tag,4,1,input)==1&&fread(&count,4,1,input)==1&&fread(&faces,4,1,input)==1&&fread(&fill,4,1,input)==1;
 if(!valid||tag!=0x31304d43||count<4||count>80||faces<4||faces>110||(fill!=0&&fill!=0xcd)){error=6;goto done;}
 valid=fread(points,16,count,input)==count&&fread(triangles,12,faces,input)==faces&&fgetc(input)==EOF;
 fclose(input);input=NULL;if(!valid){error=6;goto done;}
 unsigned edges=0;
 for(unsigned i=0;i<count;i++)for(unsigned j=0;j<4;j++)if(!isfinite(points[i][j])||fabsf(points[i][j])>100000.f||(j==3&&points[i][j]!=1.f))error=7;
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
 unsigned char *page=image+0xe58000-0x400000;memcpy(saved,page,4096);
 if(!VirtualProtect(page,4096,PAGE_READWRITE,&protection)){error=10;goto done;}protected=1;
 __try{((void (__cdecl *)(void *,void *))(image+0xb44e80-0x400000))(body,settings);}
 __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"mesh_fault=%08lx\n",GetExceptionCode());error=11;}
 if(memcmp(point_before,points,count*16)||memcmp(triangle_before,triangles,faces*12)||memcmp(settings_before,settings,sizeof(settings)))error=21;
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
 if(protected){DWORD ignored;unsigned char *restore_page=image+0xe58000-0x400000;memcpy(restore_page,saved,4096);if(!VirtualProtect(restore_page,4096,protection,&ignored)||memcmp(restore_page,saved,4096)){error=15;restored=0;}}
 unsigned code_unchanged=1;IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if((sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE)&&memcmp(image+sections[i].VirtualAddress,snapshot+sections[i].VirtualAddress,sections[i].SizeOfRawData))code_unchanged=0;
 if(!code_unchanged)error=18;
 if(!error){HANDLE output=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(output==INVALID_HANDLE_VALUE)error=19;else{DWORD n=0;if(!WriteFile(output,body,length,&n,NULL)||n!=length)error=20;CloseHandle(output);}}
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);
 printf("mesh_bytes=%u code_unchanged=%u code_replacements=0 globals_restored=%u error=%u\n",length,code_unchanged,restored,error);return (int)error;
}
