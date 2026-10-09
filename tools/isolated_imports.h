/* Bind only caller-declared named kernel32 imports in an identity-checked PE. */
#ifndef GRID_ISOLATED_IMPORTS_H
#define GRID_ISOLATED_IMPORTS_H
typedef struct {unsigned address;const char *name;DWORD original;FARPROC resolved;} GridBinding;
static int grid_imports(unsigned char *image,GridBinding *bindings,unsigned count,int restore){
 IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);size_t end=pe->OptionalHeader.SizeOfImage;
 if(!count||count>8)return 0;
 for(unsigned b=0;b<count;++b)if(bindings[b].address<0xc89000||bindings[b].address>=0xc8a000)return 0;
 if(!restore){
  IMAGE_DATA_DIRECTORY dir=pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];unsigned found[8]={0};
  if(!inside(dir.VirtualAddress,dir.Size,end))return 0;
  for(unsigned i=0;i<256;++i){size_t offset=dir.VirtualAddress+i*sizeof(IMAGE_IMPORT_DESCRIPTOR);
   if(!inside(offset,sizeof(IMAGE_IMPORT_DESCRIPTOR),dir.VirtualAddress+dir.Size))return 0;
   IMAGE_IMPORT_DESCRIPTOR *entry=(IMAGE_IMPORT_DESCRIPTOR *)(image+offset);if(!entry->Name)break;
   if(!inside(entry->Name,13,end)||memcmp(image+entry->Name,"KERNEL32.dll",13))continue;if(!entry->OriginalFirstThunk)return 0;
   for(unsigned j=0;j<4096;++j){
    if(!inside(entry->OriginalFirstThunk+j*4,4,end)||!inside(entry->FirstThunk+j*4,4,end))return 0;
    DWORD lookup=*(DWORD *)(image+entry->OriginalFirstThunk+j*4);if(!lookup)break;if(lookup&IMAGE_ORDINAL_FLAG32)continue;
    for(unsigned b=0;b<count;++b){size_t length=strlen(bindings[b].name)+1;
     if(inside(lookup+2,length,end)&&!memcmp(image+lookup+2,bindings[b].name,length)){
      if(++found[b]!=1||entry->FirstThunk+j*4!=bindings[b].address-0x400000)return 0;
      bindings[b].original=*(DWORD *)(image+entry->FirstThunk+j*4);bindings[b].resolved=GetProcAddress(GetModuleHandleW(L"kernel32.dll"),bindings[b].name);if(!bindings[b].resolved)return 0;
     }
    }
   }
  }
  for(unsigned b=0;b<count;++b)if(found[b]!=1)return 0;
 }
 DWORD old,ignored;if(!VirtualProtect(image+0xc89000-0x400000,4096,PAGE_READWRITE,&old)||old!=PAGE_READONLY)return 0;
 for(unsigned b=0;b<count;++b)*(DWORD *)(image+bindings[b].address-0x400000)=restore?bindings[b].original:(DWORD)(uintptr_t)bindings[b].resolved;
 return VirtualProtect(image+0xc89000-0x400000,4096,old,&ignored)!=0;
}
#endif
