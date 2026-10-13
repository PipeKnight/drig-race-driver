/* Complete original plane intersection and volume on owned output pools. */
#define wmain layout_probe_main
#include "probe_deformation_layout.c"
#undef wmain
#define VERTEX_BYTES (128*16)
#define TRIANGLE_BYTES (256*12)
#define ROW (4+12*20+VERTEX_BYTES+TRIANGLE_BYTES)
#define ANSWER (16+VERTEX_BYTES+TRIANGLE_BYTES)
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 if(fseek(file,0,SEEK_END)){fclose(file);return 5;}long size=ftell(file);
 if(size<8||size>8*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 5;}
 unsigned char *input=malloc((size_t)size);if(!input){fclose(file);return 6;}
 int valid=fread(input,1,(size_t)size,file)==(size_t)size;fclose(file);
 unsigned count=valid?get(input,4):0;if(!valid||get(input,0)!=0x31305644||!count||count>128||(size_t)size!=8+count*ROW){free(input);return 7;}
 for(unsigned i=0;i<count;i++){
  const unsigned char *r=input+8+i*ROW;unsigned n=get(r,0);
  if(n<3||n>12){free(input);return 9;}
  for(unsigned j=0;j<n;j++){
   const unsigned char *p=r+4+j*16;
   for(unsigned a=0;a<4;a++)if(!range(number(p,a*4),-2.f,2.f)){free(input);return 9;}
   if(number(p,12)!=0||!range(number(r,4+192+j*4),-1000.f,1000.f)){free(input);return 9;}
  }
 }
 unsigned char *results=malloc(8+count*ANSWER);if(!results){free(input);return 6;}memcpy(results,input,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(input);free(results);return 10;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *saved=malloc(image_size);if(!saved){free(input);free(results);VirtualFree(image,0,MEM_RELEASE);return 6;}memcpy(saved,image,image_size);
 DWORD previous=0,ignored;unsigned error=0,completed=0;
 if(!VirtualProtect(image+0xe58000-0x400000,4096,PAGE_READWRITE,&previous))error=11;
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=input+8+i*ROW;unsigned char *out=results+8+i*ANSWER;
  memcpy(image+0xe58000-0x400000,saved+0xe58000-0x400000,4096);
  __declspec(align(16)) unsigned char planes[32+192+32],offsets[32+48+32],vertices[32+VERTEX_BYTES+32],triangles[32+TRIANGLE_BYTES+32];
  unsigned char planes_before[sizeof(planes)],offsets_before[sizeof(offsets)];
  memset(planes,0xcd,sizeof(planes));memcpy(planes+32,r+4,192);memcpy(planes_before,planes,sizeof(planes));
  memset(offsets,0xcd,sizeof(offsets));memcpy(offsets+32,r+196,48);memcpy(offsets_before,offsets,sizeof(offsets));
  memset(vertices,0xcd,sizeof(vertices));memcpy(vertices+32,r+244,VERTEX_BYTES);
  memset(triangles,0xcd,sizeof(triangles));memcpy(triangles+32,r+244+VERTEX_BYTES,TRIANGLE_BYTES);
  unsigned vertex_count=0,triangle_count=0;double volume=0;
  __try{
   ((void (__cdecl *)(void *,void *,unsigned,unsigned *,unsigned *,void *,void *))(image+0xa15f50-0x400000))(planes+32,offsets+32,get(r,0),&vertex_count,&triangle_count,vertices+32,triangles+32);
   if(vertex_count>128||triangle_count>256)error=27;
   if(!error)volume=((double (__stdcall *)(unsigned,void *,void *))(image+0x9cc790-0x400000))(triangle_count,vertices+32,triangles+32);
  }__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  if(!sentinels(vertices,sizeof(vertices))||!sentinels(triangles,sizeof(triangles))||memcmp(planes,planes_before,sizeof(planes))||memcmp(offsets,offsets_before,sizeof(offsets))){error=21;break;}
  put(out,0,vertex_count);put(out,4,triangle_count);memcpy(out+8,&volume,8);memcpy(out+16,vertices+32,VERTEX_BYTES);memcpy(out+16+VERTEX_BYTES,triangles+32,TRIANGLE_BYTES);completed++;
 }
 if(previous){memcpy(image+0xe58000-0x400000,saved+0xe58000-0x400000,4096);if(!VirtualProtect(image+0xe58000-0x400000,4096,previous,&ignored))error=22;}
 unsigned unchanged=memcmp(image,saved,image_size)==0;if(!unchanged)error=23;
 if(!error){HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)error=24;else{DWORD written=0;unsigned length=8+count*ANSWER;if(!WriteFile(h,results,length,&written,NULL)||written!=length)error=25;if(!CloseHandle(h))error=25;if(error)_wremove(argv[3]);}}
 printf("volume_cases=%u completed=%u owned_canaries=1 full_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(input);free(results);free(saved);VirtualFree(image,0,MEM_RELEASE);return error;
}
