/* Whole B7B5E0 with impact pools only: original bias/solve/pose, no stubs. */
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
__declspec(noinline) static void invoke(void *function,void *manager){
 __asm {mov ecx,manager
        push 0
        call function}
}
static int finite_matrix(const unsigned char *p){for(unsigned i=0;i<16;++i)if(!isfinite(*(const float *)(p+i*4)))return 0;return 1;}
static int finite_row(const unsigned char *r){
 if(!finite_matrix(r)||!finite_matrix(r+0x40))return 0;const unsigned offsets[]={0x84,0x88,0x8c,0x90,0x94,0x98,0x9c,0xac,0xb0};
 for(unsigned i=0;i<sizeof(offsets)/sizeof(offsets[0]);++i)if(!isfinite(*(const float *)(r+offsets[i])))return 0;return 1;
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
 if(fseek(input,0,SEEK_END)||ftell(input)>32*1024*1024||fseek(input,0,SEEK_SET)||fread(&magic,4,1,input)!=1||magic!=0x31435049||fread(&total,4,1,input)!=1||!total||total>1024){error=6;goto finish;}
 fwrite(&magic,4,1,output);fwrite(&total,4,1,output);
 for(unsigned c=0;c<total;++c){
  unsigned h[6],contact_values[256][2],ph[16][2];float parameters[4];int active=0;
  __declspec(align(16)) unsigned char rows[256][192],contacts[256][128]={0},manager[0x120]={0},bodies[16][0x430],shapes[16][8][0x220],groups[16][0x40]={0};
  __declspec(align(16)) float workspace[16][8];void *pool_two[256],*pool_single[256],*body_pointers[16];
  if(fread(h,4,6,input)!=6||fread(parameters,4,4,input)!=4||h[0]>256||h[1]>h[0]||h[2]>16||h[3]>32||fread(rows,192,h[0],input)!=h[0]||fread(contact_values,8,h[0],input)!=h[0]||fread(workspace,32,h[2],input)!=h[2]){error=7;break;}
  for(unsigned i=0;i<4;++i)if(!isfinite(parameters[i]))error=8;
  for(unsigned i=0;i<h[2];++i)for(unsigned j=0;j<8;++j)if(!isfinite(workspace[i][j]))error=8;
  for(unsigned i=0;i<h[0];++i){if(!finite_row(rows[i])||*(float *)(rows[i]+0x88)>*(float *)(rows[i]+0x8c)||contact_values[i][0]>1||!isfinite(*(float *)(contact_values[i]+1)))error=8;
   if(!contact_values[i][0]){float bias=_mm_cvtss_f32(_mm_mul_ss(_mm_sub_ss(_mm_set_ss(*(float *)(contact_values[i]+1)),_mm_set_ss(parameters[2])),_mm_set_ss(parameters[1])));if(!isfinite(bias))error=8;if(bias>=0.f)active=1;}
   contacts[i][0xe]=(unsigned char)contact_values[i][0];memcpy(contacts[i]+0x38,contact_values[i]+1,4);
   if(i<h[1])pool_two[i]=contacts[i];else pool_single[i-h[1]]=contacts[i];
  }
  for(unsigned group=0;group<2&&!error;++group){unsigned start=group?h[1]:0,end=group?h[0]:h[1];while(start<end){unsigned length=*(unsigned *)(rows[start]+0xb4),a=*(unsigned *)(rows[start]+0xa0),b=*(unsigned *)(rows[start]+0xa4);if(!length||length>end-start||a>=h[2]||(!group&&b>=h[2])){error=8;break;}for(unsigned j=start;j<start+length;++j)if(*(unsigned *)(rows[j]+0xa0)!=a||(!group&&*(unsigned *)(rows[j]+0xa4)!=b))error=8;start+=length;}}
  const unsigned pointers[]={0x328,0x32c,0x334,0x338,0x33c,0x340};
  for(unsigned i=0;i<h[2]&&!error;++i){
   if(fread(ph[i],4,2,input)!=2||ph[i][0]>1||ph[i][1]>8||fread(bodies[i],1,0x424,input)!=0x424||fread(shapes[i],0x220,ph[i][1],input)!=ph[i][1]){error=7;break;}
   for(unsigned j=0;j<6;++j)if(*(unsigned *)(bodies[i]+pointers[j]))error=8;
   if(!finite_matrix(bodies[i]+0x90)||!finite_matrix(bodies[i]+0x150)||!finite_matrix(bodies[i]+0x190))error=8;
   for(unsigned j=0;j<4;++j)if(!isfinite(*(float *)(bodies[i]+0x230+j*4)))error=8;
   for(unsigned j=0;j<ph[i][1];++j)if(*(unsigned *)(shapes[i][j]+0xe0)||shapes[i][j][0x214]>1||!finite_matrix(shapes[i][j]+0xf0))error=8;
   *(void **)(groups[i]+0x28)=ph[i][0]?bodies[i]:NULL;*(void **)(bodies[i]+0x328)=ph[i][1]?shapes[i]:NULL;*(void **)(bodies[i]+0x32c)=groups[i];
   for(unsigned j=0;j<4;++j)*(void **)(bodies[i]+0x334+j*4)=bodies[i]+(j?0x180+j*16:0x1c0);
   for(unsigned j=0;j<ph[i][1];++j)*(void **)(shapes[i][j]+0xe0)=j+1<ph[i][1]?shapes[i][j+1]:NULL;
   body_pointers[i]=bodies[i];
  }
  if(error)break;
  *(void **)(manager+0x80)=workspace;*(void **)(manager+0x8c)=rows;*(void **)(manager+0x90)=body_pointers;*(void **)(manager+0x98)=pool_single;*(void **)(manager+0xa0)=pool_two;
  *(unsigned *)(manager+0xa8)=h[2];*(unsigned *)(manager+0xac)=h[0]-h[1];*(unsigned *)(manager+0xb4)=h[1];*(unsigned *)(manager+0xbc)=h[0];*(unsigned *)(manager+0xc4)=h[1];
  *(unsigned *)(manager+0xc0)=h[4];*(unsigned *)(manager+0xc8)=h[5];memcpy(manager+0xe0,parameters,12);memcpy(manager+0xf0,parameters+3,4);*(unsigned *)(manager+0xf4)=h[3];
  __try{invoke(image+0xb7b5e0-0x400000,manager);}__except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"impact_correction_fault=%08lx\n",GetExceptionCode());error=9;}
  if(error)break;for(unsigned i=0;i<h[0];++i)if(!finite_row(rows[i]))error=10;
  for(unsigned i=0;i<h[2];++i){for(unsigned j=0;j<8;++j)if(!isfinite(workspace[i][j]))error=10;
   if(active){float *a=workspace[i]+4;__m128 p=_mm_mul_ps(_mm_load_ps(a),_mm_load_ps(a));__declspec(align(16)) float square[4];_mm_store_ps(square,p);volatile float lo=square[1]+square[0],hi=square[3]+square[2],squared=lo+hi;
    if(!isfinite(squared)||squared>4096.f||!finite_matrix(bodies[i]+0x190)||!finite_matrix(bodies[i]+0x1d0)||!finite_matrix(bodies[i]+0xd0)||!finite_matrix(bodies[i]+0x110))error=10;
    for(unsigned j=0;j<ph[i][1];++j)if(!finite_matrix(shapes[i][j]+0x130))error=10;
   }
   for(unsigned j=0;j<6;++j)*(unsigned *)(bodies[i]+pointers[j])=0;
   for(unsigned j=0;j<ph[i][1];++j)*(unsigned *)(shapes[i][j]+0xe0)=0;
  }
  if(error)break;unsigned out[4]={h[0],h[2],*(unsigned *)(manager+0xc0),*(unsigned *)(manager+0xc8)};fwrite(out,4,4,output);fwrite(rows,192,h[0],output);fwrite(workspace,32,h[2],output);
  for(unsigned i=0;i<h[2];++i){fwrite(ph[i],4,2,output);fwrite(bodies[i],1,0x424,output);fwrite(shapes[i],0x220,ph[i][1],output);}++cases;
 }
 if(!error&&fgetc(input)!=EOF)error=11;
finish:
 for(unsigned i=0;i<pe->FileHeader.NumberOfSections;++i)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;if(memcmp(snapshot+sections[i].VirtualAddress,image+sections[i].VirtualAddress,size))error=12;}
 for(unsigned i=0;i<4;++i){DWORD ignored;memcpy(image+pages[i]-0x400000,snapshot+pages[i]-0x400000,0x1000);if(!VirtualProtect(image+pages[i]-0x400000,0x1000,protections[i],&ignored))error=17;}
 if(ferror(input)||ferror(output))error=13;int ci=fclose(input),co=fclose(output);if(ci||co)error=13;
 free(snapshot);VirtualFree(image,0,MEM_RELEASE);printf("impact_correction_cases=%u error=%u code_unchanged=%u no_imports=1 globals_restored=1\n",cases,error,error!=12);return (int)error;
}
