/* Complete cold reset -> layout -> mode-zero node/edge construction.
   Shared admission/mapping only; no original helper or instruction replacement. */
#define wmain layout_probe_main
#include "probe_deformation_layout.c"
#undef wmain
#define SETTINGS 0x1054
#define RESULT (3*DESCRIPTOR+SETTINGS)
static int settings_admitted(const unsigned char *r){
 unsigned n=get(r,0x1000);if(n>32)return 0;
 for(unsigned i=0;i<n;i++){
  const unsigned char *b=r+i*0x80;
  for(unsigned j=0;j<16;j++)if(!range(number(b,j*4),-100.f,100.f))return 0;
  if(number(b,12)!=0||number(b,28)!=0||number(b,44)!=0||number(b,60)!=1.f)return 0;
  for(unsigned j=0;j<3;j++)if(!range(number(b,0x40+j*4),.0001f,100.f))return 0;
  if(number(b,0x4c)!=0)return 0;
  for(unsigned j=0;j<4;j++)if(!range(number(b,0x50+j*4),0.f,100.f))return 0;
 }
 for(unsigned o=0x1014;o<SETTINGS;o+=4)if(!range(number(r,o),-10000.f,10000.f))return 0;
 return number(r,0x1014)>0&&number(r,0x1018)>0&&number(r,0x1028)>=0;
}
static int stage(unsigned char *out,const unsigned char *descriptor,const unsigned char *settings,unsigned stage_index,uintptr_t reference){
 memcpy(out,descriptor,DESCRIPTOR);
 if(get(out,0xa484)!=(unsigned)(reference+0x9a0c90-0x400000))return 0;
 put(out,0xa484,0x9a0c90);
 if(get(out,0xa4f8)!=(stage_index?(unsigned)(uintptr_t)settings:0))return 0;
 put(out,0xa4f8,0);return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 if(fseek(file,0,SEEK_END)){fclose(file);return 5;}long size=ftell(file);
 if(size<8||size>8*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 5;}
 unsigned char *input=malloc((size_t)size);if(!input){fclose(file);return 6;}
 int valid=fread(input,1,(size_t)size,file)==(size_t)size;fclose(file);
 unsigned count=valid?get(input,4):0;if(!valid||get(input,0)!=0x31304947||!count||count>32){free(input);return 7;}
 size_t cursor=8;unsigned error=0,completed=0,settings_changes=0;
 for(unsigned i=0;i<count;i++){
  if(!inside(cursor,32+DESCRIPTOR+SETTINGS,(size_t)size)){error=8;break;}
  const unsigned char *r=input+cursor;unsigned n=get(r,0);
  if(n>32||!inside(cursor,32+DESCRIPTOR+n*32+SETTINGS,(size_t)size)){error=8;break;}
  if(!admitted(r)||get(r,4)||get(r+32,0xa410)||get(r+32,0xa4f8)||!settings_admitted(r+32+DESCRIPTOR+n*32)){error=9;break;}
  cursor+=32+DESCRIPTOR+n*32+SETTINGS;
 }
 if(!error&&cursor!=(size_t)size)error=8;if(error){free(input);return error;}
 unsigned char *results=malloc(8+count*RESULT);if(!results){free(input);return 6;}memcpy(results,input,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(input);free(results);return 10;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *saved=malloc(image_size);if(!saved){free(input);free(results);VirtualFree(image,0,MEM_RELEASE);return 6;}memcpy(saved,image,image_size);
 const unsigned pages[]={0xcaf000,0xcb0000,0xce2000,0xcf7000,0xe4f000,0xe58000,0xe8f000};DWORD previous[7],ignored;unsigned protected=0;
 for(unsigned i=0;i<7;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,previous+i)){error=11;break;}protected++;}
 cursor=8;
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=input+cursor;unsigned n=get(r,0);unsigned char *out=results+8+i*RESULT;
  for(unsigned j=0;j<7;j++)memcpy(image+pages[j]-0x400000,saved+pages[j]-0x400000,4096);
  const unsigned addresses[]={0xcb0de4,0xcafe5c,0xce2bb4,0xcb0948,0xcf7718};
  for(unsigned j=0;j<5;j++)put(image,addresses[j]-0x400000,get(r,12+j*4));image[0xe4fc34-0x400000]=(unsigned char)get(r,8);
  __declspec(align(16)) unsigned char descriptor[32+DESCRIPTOR+32],owner[32+0x450+32],shapes[32][32+0x130+32],settings[32+SETTINGS+32];
  unsigned char owner_before[sizeof(owner)],shapes_before[sizeof(shapes)],settings_before[SETTINGS];
  memset(descriptor,0xcd,sizeof(descriptor));memcpy(descriptor+32,r+32,DESCRIPTOR);
  memset(owner,0xcd,sizeof(owner));memset(owner+32,0,0x450);memset(shapes,0xcd,sizeof(shapes));
  memset(settings,0xcd,sizeof(settings));memcpy(settings+32,r+32+DESCRIPTOR+n*32,SETTINGS);memcpy(settings_before,settings+32,SETTINGS);
  put(owner+32,0x44,n);put(owner+32,0x44c,(unsigned)(uintptr_t)(settings+32));
  for(unsigned j=0;j<n;j++){put(owner+32,0x48+j*4,(unsigned)(uintptr_t)(shapes[j]+32));memcpy(shapes[j]+32+0x110,r+32+DESCRIPTOR+j*32,32);}
  memcpy(owner_before,owner,sizeof(owner));memcpy(shapes_before,shapes,sizeof(shapes));
  __try{
   ((void (__fastcall *)(void *,void *))(image+0x9a2e30-0x400000))(descriptor+32,NULL);
   if(!stage(out,descriptor+32,settings+32,0,(uintptr_t)image))error=26;
   if(!error){((void (__fastcall *)(void *,void *,void *))(image+0x9a2a70-0x400000))(descriptor+32,NULL,owner+32);if(!stage(out+DESCRIPTOR,descriptor+32,settings+32,1,(uintptr_t)image))error=26;}
   if(!error){((void (__fastcall *)(void *,void *,void *))(image+0x9fed60-0x400000))(descriptor+32,NULL,NULL);if(!stage(out+2*DESCRIPTOR,descriptor+32,settings+32,2,(uintptr_t)image))error=26;}
  }__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  if(!sentinels(descriptor,sizeof(descriptor))||!sentinels(settings,sizeof(settings))||memcmp(owner,owner_before,sizeof(owner))||memcmp(shapes,shapes_before,sizeof(shapes))){error=21;break;}
  settings_changes+=memcmp(settings+32,settings_before,SETTINGS)!=0;
  memcpy(out+3*DESCRIPTOR,settings+32,SETTINGS);completed++;
  cursor+=32+DESCRIPTOR+n*32+SETTINGS;
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,previous[i],&ignored))error=22;}
 unsigned unchanged=memcmp(image,saved,image_size)==0;if(!unchanged)error=23;
 if(!error){HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)error=24;else{DWORD written=0;unsigned length=8+count*RESULT;if(!WriteFile(h,results,length,&written,NULL)||written!=length)error=25;if(!CloseHandle(h))error=25;if(error)_wremove(argv[3]);}}
 printf("initialization_cases=%u completed=%u settings_changed_cases=%u owned_canaries=1 full_image_unchanged=%u error=%u\n",count,completed,settings_changes,unchanged,error);
 free(input);free(results);free(saved);VirtualFree(image,0,MEM_RELEASE);return error;
}
