/* Whole B77850 -> registered 9A0C90, bounded first pool and empty second. */
#define DEFORMATION_CAP_EMBEDDED
#include "probe_deformation_contact_limit.c"
#undef ROW
#undef ANSWER
#define ROW (16+DESCRIPTOR+LINKED+16+96+64*0x80)
#define ANSWER (16+DESCRIPTOR+LINKED+64*0x80)
static int preparation_admitted(const unsigned char *r){
 unsigned count=get(r,0);const unsigned char *d=r+16,*b=d+DESCRIPTOR,*center=b+LINKED,*p=center+16,*contacts=p+96;
 if(count>64||get(r,4)>count||get(r,8)>10000||get(r,12)>10000||get(b,0x418)||get(b,0x420))return 0;
 unsigned char scratch[DESCRIPTOR+LINKED+0x80+16+96];
 memcpy(scratch,d,DESCRIPTOR+LINKED);memset(scratch+DESCRIPTOR+LINKED,0,0x80);scratch[DESCRIPTOR+LINKED+0xc]=1;
 memcpy(scratch+DESCRIPTOR+LINKED+0x80,center,16+96);if(!cap_admitted(scratch))return 0;
 for(unsigned i=0;i<count;i++){
  const unsigned char *c=contacts+i*0x80;if(get(c,4)>1)return 0;
  if(!get(c,4)){memcpy(scratch+DESCRIPTOR+LINKED,c,0x80);if(!cap_admitted(scratch)||!isfinite(number(c,0x3c)))return 0;}
 }
 return 1;
}
int wmain(int argc,wchar_t **argv){
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);if(argc!=4)return 2;
 unsigned control;if(_controlfp_s(&control,_PC_53|_RC_NEAR,_MCW_PC|_MCW_RC))return 3;
 FILE *file=NULL;if(_wfopen_s(&file,argv[2],L"rb")||!file)return 4;
 if(fseek(file,0,SEEK_END)){fclose(file);return 5;}long size=ftell(file);
 if(size<8||size>8*1024*1024||fseek(file,0,SEEK_SET)){fclose(file);return 5;}
 unsigned char *input=malloc((size_t)size);if(!input){fclose(file);return 6;}int valid=fread(input,1,(size_t)size,file)==(size_t)size;fclose(file);
 unsigned count=valid?get(input,4):0;if(!valid||get(input,0)!=0x32304344||!count||count>128||(size_t)size!=8+count*ROW){free(input);return 7;}
 for(unsigned i=0;i<count;i++)if(!preparation_admitted(input+8+i*ROW)){free(input);return 9;}
 unsigned char *results=malloc(8+count*ANSWER);if(!results){free(input);return 6;}memcpy(results,input,8);
 unsigned char *image=map_reference(argv[1]);if(!image){free(input);free(results);return 10;}
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);unsigned image_size=pe->OptionalHeader.SizeOfImage;
 unsigned char *saved=malloc(image_size);if(!saved){free(input);free(results);VirtualFree(image,0,MEM_RELEASE);return 6;}memcpy(saved,image,image_size);
 const unsigned pages[]={0xcaf000,0xcb0000,0xcb3000,0xe4f000,0xe51000,0xe5a000,0xe85000};DWORD previous[7],ignored;unsigned protected=0,error=0,completed=0;
 for(unsigned i=0;i<7;i++){if(!VirtualProtect(image+pages[i]-0x400000,4096,PAGE_READWRITE,previous+i)){error=11;break;}protected++;}
 for(unsigned i=0;i<count&&!error;i++){
  const unsigned char *r=input+8+i*ROW,*source=r+16,*source_center=source+DESCRIPTOR+LINKED,*p=source_center+16,*source_contacts=p+96;unsigned char *out=results+8+i*ANSWER;
  unsigned used=get(r,0);
  for(unsigned j=0;j<7;j++)memcpy(image+pages[j]-0x400000,saved+pages[j]-0x400000,4096);
  const unsigned addresses[]={0xcafbdc,0xcb093c,0xe4fc2c,0xe4fc3c,0xcb3ba8,0xe51d40,0xe4fc30};
  for(unsigned j=0;j<7;j++)put(image,addresses[j]-0x400000,get(p,j*4));memcpy(image+0xe4fc48-0x400000,p+28,64);image[0xe85fad-0x400000]=(unsigned char)get(p,92);
  __declspec(align(16)) unsigned char d[32+DESCRIPTOR+32],b[32+LINKED+44],c[32+64*0x80+32],center[32+16+32],manager[32+0x100+32],list[32+64*4+32];
  unsigned char body_before[LINKED],manager_before[sizeof(manager)],list_before[sizeof(list)],center_before[sizeof(center)];
  memset(d,0xcd,sizeof(d));memcpy(d+32,source,DESCRIPTOR);memset(b,0xcd,sizeof(b));memcpy(b+32,source+DESCRIPTOR,LINKED);
  memset(c,0xcd,sizeof(c));memcpy(c+32,source_contacts,64*0x80);memset(center,0xcd,sizeof(center));memcpy(center+32,source_center,16);memcpy(center_before,center,sizeof(center));
  put(b+32,0x334,(unsigned)(uintptr_t)(center+32));put(b+32,0x418,(unsigned)(uintptr_t)(image+0x9a0c90-0x400000));put(b+32,0x420,(unsigned)(uintptr_t)(d+32));memcpy(body_before,b+32,LINKED);
  memset(list,0xcd,sizeof(list));
  for(unsigned j=0;j<used;j++){put(list+32,j*4,(unsigned)(uintptr_t)(c+32+j*0x80));put(c+32+j*0x80,4,get(source_contacts+j*0x80,4)?0:(unsigned)(uintptr_t)(b+32));}
  memcpy(list_before,list,sizeof(list));memset(manager,0xcd,sizeof(manager));memset(manager+32,0,0x100);
  put(manager+32,0x98,(unsigned)(uintptr_t)(list+32));put(manager+32,0xac,used);memcpy(manager_before,manager,sizeof(manager));
  unsigned cursors[]={0xcdcdcdcd,get(r,4),0xcdcdcdcd,0,0xcdcdcdcd};unsigned done=0;
  __try{done=((unsigned (__fastcall *)(void *,void *,void *,float,unsigned,unsigned,unsigned *,unsigned *))(image+0xb77850-0x400000))(manager+32,NULL,b+32,1.f,get(r,8),get(r,12),cursors+1,cursors+3)&255;}__except(fault(GetExceptionInformation())){error=20;}
  if(error)break;
  if(!sentinels(d,sizeof(d))||!sentinels(c,sizeof(c))||memcmp(b+32,body_before,LINKED)||memcmp(center,center_before,sizeof(center))||memcmp(manager,manager_before,sizeof(manager))||memcmp(list,list_before,sizeof(list))||cursors[0]!=0xcdcdcdcd||cursors[2]!=0xcdcdcdcd||cursors[4]!=0xcdcdcdcd||cursors[1]>used||cursors[3]||done>1){error=21;break;}
  for(unsigned j=0;j<32;j++)if(b[j]!=0xcd||b[32+LINKED+j]!=0xcd)error=21;if(error)break;
  for(unsigned j=0;j<used;j++){unsigned expected=get(source_contacts+j*0x80,4)?0:(unsigned)(uintptr_t)(b+32);if(get(c+32+j*0x80,4)!=expected)error=21;put(c+32+j*0x80,4,get(source_contacts+j*0x80,4));}if(error)break;
  put(b+32,0x334,0);put(b+32,0x418,0);put(b+32,0x420,0);
  put(out,0,cursors[1]);put(out,4,cursors[3]);put(out,8,done);put(out,12,0);
  memcpy(out+16,d+32,DESCRIPTOR);memcpy(out+16+DESCRIPTOR,b+32,LINKED);memcpy(out+16+DESCRIPTOR+LINKED,c+32,64*0x80);completed++;
 }
 for(unsigned i=0;i<protected;i++){memcpy(image+pages[i]-0x400000,saved+pages[i]-0x400000,4096);if(!VirtualProtect(image+pages[i]-0x400000,4096,previous[i],&ignored))error=22;}
 unsigned unchanged=memcmp(image,saved,image_size)==0;if(!unchanged)error=23;
 if(!error){HANDLE h=CreateFileW(argv[3],GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);if(h==INVALID_HANDLE_VALUE)error=24;else{DWORD written=0;unsigned length=8+count*ANSWER;if(!WriteFile(h,results,length,&written,NULL)||written!=length)error=25;if(!CloseHandle(h))error=25;if(error)_wremove(argv[3]);}}
 printf("preparation_cases=%u completed=%u owned_canaries=1 full_image_unchanged=%u error=%u\n",count,completed,unchanged,error);
 free(input);free(results);free(saved);VirtualFree(image,0,MEM_RELEASE);return error;
}
