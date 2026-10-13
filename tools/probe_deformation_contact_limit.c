/* Whole original 9A0C90, first registered owner and static counterpart. */
#define wmain layout_probe_main
#include "probe_deformation_layout.c"
#undef wmain
#define LINKED 0x424
#define ROW (DESCRIPTOR+LINKED+0x80+16+96)
#define ANSWER (DESCRIPTOR+LINKED+0x80+8)
static int cap_admitted(const unsigned char *r){
 const unsigned char *b=r+DESCRIPTOR,*c=b+LINKED,*center=c+0x80,*p=center+16;
 unsigned nx=get(r,0xa414),ny=get(r,0xa418),nz=get(r,0xa41c);
 if(nx<2||nx>16||ny<2||ny>16||nz<2||nz>16||nx*ny*nz>400||get(r,0xa4ec)>=10||!c[0xc]||get(c,4)||get(c,8)||get(b,0x334)||get(p,92)>1||number(p,20)!=FLT_MAX)return 0;
 for(unsigned i=0;i<23;i++)if(i!=5&&!range(number(p,i*4),-1e6f,1e6f))return 0;
 const unsigned offsets[]={0x1d0,0x1e0,0x1f0,0x200,0x210,0x220,0x360};
 for(unsigned i=0;i<7;i++)for(unsigned a=0;a<4;a++)if(!range(number(b,offsets[i]+a*4),-1e6f,1e6f))return 0;
 for(unsigned a=0;a<4;a++)if(!range(number(center,a*4),-1e6f,1e6f)||!range(number(c,0x50+a*4),-1e6f,1e6f)||!range(number(c,0x60+a*4),-1e6f,1e6f)||!isfinite(number(r,0xa430+a*4))||!isfinite(number(r,0xa4b4+a*4)))return 0;
 if(!range(number(b,0x3f0),.0001f,10000.f)||!isfinite(number(c,0x38))||!isfinite(number(r,0xa490))||!isfinite(number(r,0xa494))||!isfinite(number(r,0xa49c))||!isfinite(number(r,0xa420))||number(r,0xa420)<=0)return 0;
 for(unsigned i=0;i<10;i++)if(!isfinite(number(r,0xa4c4+i*4)))return 0;
 for(unsigned i=0;i<nx*ny*nz;i++)for(unsigned a=0;a<5;a++)if(!range(number(r,0x2580+i*80+(a==4?0x40:a*4)),-1e6f,1e6f))return 0;
 return 1;
}
#ifndef DEFORMATION_CAP_EMBEDDED
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 if(fseek(file,0,SEEK_END)){fclose(file);return 5;}long size=ftell(file);
 if(size<8||size>8*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 5;}
 unsigned char *input=malloc((size_t)size);if(!input){fclose(file);return 6;}int valid=fread(input,1,(size_t)size,file)==(size_t)size;fclose(file);
 unsigned count=valid?get(input,4):0;if(!valid||get(input,0)!=0x31304344||!count||count>128||(size_t)size!=8+count*ROW){free(input);return 7;}
 for(unsigned i=0;i<count;i++)if(!cap_admitted(input+8+i*ROW)){free(input);return 9;}
 unsigned char *results=malloc(8+count*ANSWER);if(!results){free(input);return 6;}memcpy(results,input,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(input);free(results);return 10;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *saved=malloc(image_size);if(!saved){free(input);free(results);VirtualFree(image,0,MEM_RELEASE);return 6;}memcpy(saved,image,image_size);
 const unsigned pages[]={0xcaf000,0xcb0000,0xcb3000,0xe4f000,0xe51000,0xe5a000,0xe85000};DWORD previous[7],ignored;unsigned protected=0,error=0,completed=0;
 for(unsigned i=0;i<7;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,previous+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=input+8+i*ROW,*p=r+DESCRIPTOR+LINKED+0x80+16;unsigned char *out=results+8+i*ANSWER;
  for(unsigned j=0;j<7;j++)memcpy(image+pages[j]-0x400000,saved+pages[j]-0x400000,4096);
  const unsigned addresses[]={0xcafbdc,0xcb093c,0xe4fc2c,0xe4fc3c,0xcb3ba8,0xe51d40,0xe4fc30};
  for(unsigned j=0;j<7;j++)put(image,addresses[j]-0x400000,get(p,j*4));memcpy(image+0xe4fc48-0x400000,p+28,64);image[0xe85fad-0x400000]=(unsigned char)get(p,92);
  __declspec(align(16)) unsigned char d[32+DESCRIPTOR+32],b[32+LINKED+44],c[32+0x80+32],center[32+16+32];
  unsigned char body_before[LINKED],center_before[sizeof(center)];
  memset(d,0xcd,sizeof(d));memcpy(d+32,r,DESCRIPTOR);memset(b,0xcd,sizeof(b));memcpy(b+32,r+DESCRIPTOR,LINKED);
  memset(c,0xcd,sizeof(c));memcpy(c+32,r+DESCRIPTOR+LINKED,0x80);memset(center,0xcd,sizeof(center));memcpy(center+32,r+DESCRIPTOR+LINKED+0x80,16);memcpy(center_before,center,sizeof(center));
  put(b+32,0x334,(unsigned)(uintptr_t)(center+32));memcpy(body_before,b+32,LINKED);put(c+32,4,(unsigned)(uintptr_t)(b+32));
  double cap=0;
  __try{cap=((double (__cdecl *)(void *,void *,unsigned,void *))(image+0x9a0c90-0x400000))(b+32,c+32,0x7fc01234,d+32);}__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  if(!sentinels(d,sizeof(d))||!sentinels(c,sizeof(c))||memcmp(b+32,body_before,LINKED)||memcmp(center,center_before,sizeof(center))||get(c+32,4)!=(unsigned)(uintptr_t)(b+32)){error=21;break;}
  for(unsigned j=0;j<32;j++)if(b[j]!=0xcd||b[32+LINKED+j]!=0xcd)error=21;if(error)break;
  put(b+32,0x334,0);put(c+32,4,0);memcpy(out,d+32,DESCRIPTOR);memcpy(out+DESCRIPTOR,b+32,LINKED);memcpy(out+DESCRIPTOR+LINKED,c+32,0x80);memcpy(out+DESCRIPTOR+LINKED+0x80,&cap,8);completed++;
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,previous[i],&ignored))error=22;}
 unsigned unchanged=memcmp(image,saved,image_size)==0;if(!unchanged)error=23;
 if(!error){HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)error=24;else{DWORD written=0;unsigned length=8+count*ANSWER;if(!WriteFile(h,results,length,&written,NULL)||written!=length)error=25;if(!CloseHandle(h))error=25;if(error)_wremove(argv[3]);}}
 printf("contact_limit_cases=%u completed=%u owned_canaries=1 full_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(input);free(results);free(saved);VirtualFree(image,0,MEM_RELEASE);return error;
}
#endif
