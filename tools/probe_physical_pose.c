/* Whole B13B00/B13720/B12B10/B0F730/B100F0, original rotation/cross helpers.
   Private mapped data only; no imports, entry point, stubs or code writes. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <wincrypt.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <wchar.h>
#include <xmmintrin.h>
#include "isolated_reference.h"
__declspec(noinline) static void correct(void *function,void *body,void *linear,void *angular){
 __asm {mov ecx,body
        push angular
        push linear
        call function}
}
static int finite_matrix(const unsigned char *p){
 for(unsigned i=0;i<16;++i)if(!isfinite(*(const float *)(p+i*4)))return 0;return 1;
}
static float dot(const float *a,const float *b){
 volatile float p0=a[0]*b[0],p1=a[1]*b[1],p2=a[2]*b[2],p3=a[3]*b[3];
 volatile float lo=p1+p0,hi=p3+p2;return lo+hi;
}
static int nondegenerate(const unsigned char *body){
 const float *a=(const float *)(body+0x190),*b=(const float *)(body+0x1a0);
 float squared=dot(a,a);if(!isfinite(squared)||squared<1.17549435e-38f)return 0;
 float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(squared)));
 volatile float root=seed*squared,product=root*seed,half=seed*.5f,remainder=1.f-product,refined=remainder*half,inverse=refined+seed;
 float first[4],second[4];for(unsigned i=0;i<4;++i)first[i]=inverse*a[i];
 float projection=dot(first,b);for(unsigned i=0;i<4;++i){volatile float v=projection*first[i];second[i]=b[i]-v;}
 squared=dot(second,second);return isfinite(squared)&&squared>=1.17549435e-38f;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned char *image=map_reference(argv[1]);if(!image)return 3;
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
 unsigned char *snapshot=malloc(pe->OptionalHeader.SizeOfImage);if(!snapshot)return 4;memcpy(snapshot,image,pe->OptionalHeader.SizeOfImage);
 const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4];
 for(unsigned i=0;i<4;++i)if(!VirtualProtect(image+pages[i]-0x400000,0x1000,PAGE_READWRITE,protections+i))return 16;
 FILE *input=NULL,*output=NULL;unsigned magic,total=0,cases=0,error=0;
 if(_wfopen_s(&input,argv[2],L"rb")||!input||_wfopen_s(&output,argv[3],L"wbx")||!output)return 5;
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31435050||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;++c){
  unsigned h[2];__declspec(align(16)) unsigned char body[0x430],shapes[64][0x220],group[0x40]={0};__declspec(align(16)) float correction[8];
  if(fread(h,4,2,input)!=2||h[0]>1||h[1]>64||fread(body,1,0x424,input)!=0x424||fread(correction,4,8,input)!=8||fread(shapes,0x220,h[1],input)!=h[1]){error=7;break;}
  const unsigned pointers[]={0x328,0x32c,0x334,0x338,0x33c,0x340};
  for(unsigned i=0;i<6;++i)if(*(unsigned *)(body+pointers[i]))error=8;
  if(!finite_matrix(body+0x90)||!finite_matrix(body+0x150)||!finite_matrix(body+0x190))error=8;
  for(unsigned i=0;i<4;++i)if(!isfinite(*(float *)(body+0x230+i*4)))error=8;
  for(unsigned i=0;i<8;++i)if(!isfinite(correction[i]))error=8;
  float angular=dot(correction+4,correction+4);if(!isfinite(angular)||angular>4096.f)error=8;
  if(h[0]&&!nondegenerate(body))error=8;
  for(unsigned i=0;i<h[1];++i)if(*(unsigned *)(shapes[i]+0xe0)||shapes[i][0x214]>1||!finite_matrix(shapes[i]+0xf0))error=8;
  if(error)break;
  *(void **)(group+0x28)=h[0]?body:NULL;*(void **)(body+0x328)=h[1]?shapes:NULL;*(void **)(body+0x32c)=group;
  for(unsigned i=0;i<4;++i)*(void **)(body+0x334+i*4)=body+(i?0x180+i*16:0x1c0);
  for(unsigned i=0;i<h[1];++i)*(void **)(shapes[i]+0xe0)=i+1<h[1]?shapes[i+1]:NULL;
  __try{correct(image+0xb13b00-0x400000,body,correction,correction+4);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"physical_pose_fault=%08lx\n",GetExceptionCode());error=9;}
  if(error)break;
  for(unsigned i=0;i<6;++i)*(unsigned *)(body+pointers[i])=0;
  for(unsigned i=0;i<h[1];++i)*(unsigned *)(shapes[i]+0xe0)=0;
  if(!finite_matrix(body+0xd0)||!finite_matrix(body+0x110)||!finite_matrix(body+0x190)||!finite_matrix(body+0x1d0))error=10;
  for(unsigned i=0;i<h[1];++i)if(!finite_matrix(shapes[i]+0x130))error=10;
  if(error)break;fwrite(h,4,2,output);fwrite(body,1,0x424,output);fwrite(shapes,0x220,h[1],output);++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(snapshot+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 for(unsigned i=0;i<4;++i){DWORD ignored;memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,0x1000);if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=17;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);printf("physical_pose_cases=%u error=%u code_unchanged=%u no_imports=1 globals_restored=1\n",cases,error,error!=12);return (int)error;
}
