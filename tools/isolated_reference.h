/* Shared identity-checked private PE mapper. No entry point/import/TLS execution. */
#ifndef GRID_ISOLATED_REFERENCE_H
#define GRID_ISOLATED_REFERENCE_H
static int inside(size_t start,size_t count,size_t end) { return start<=end && count<=end-start; }
static unsigned bits(float value) { unsigned result; memcpy(&result,&value,4); return result; }

static int identity(const unsigned char *bytes,DWORD length) {
    const char *expected="aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514";
    HCRYPTPROV provider=0; HCRYPTHASH hash=0;
    unsigned char digest[32]; DWORD size=32; char hex[65]; int ok=0;
    if (!CryptAcquireContextW(&provider,NULL,NULL,PROV_RSA_AES,CRYPT_VERIFYCONTEXT)) goto done;
    if (!CryptCreateHash(provider,CALG_SHA_256,0,0,&hash)) goto done;
    if (!CryptHashData(hash,bytes,length,0) || !CryptGetHashParam(hash,HP_HASHVAL,digest,&size,0) || size!=32) goto done;
    for (unsigned i=0;i<32;i++) sprintf_s(hex+i*2,3,"%02x",digest[i]);
    ok=strcmp(hex,expected)==0;
done:
    if(hash) CryptDestroyHash(hash);
    if(provider) CryptReleaseContext(provider,0);
    return ok;
}

static unsigned char *map_reference(const wchar_t *path) {
    FILE *input=NULL; unsigned char *raw=NULL,*image=NULL;
    long length; IMAGE_DOS_HEADER *dos; IMAGE_NT_HEADERS32 *pe; unsigned stage=1;
    if (_wfopen_s(&input,path,L"rb") || !input) return NULL;
    if(fseek(input,0,SEEK_END) || (length=ftell(input))<64 || length>64*1024*1024 || fseek(input,0,SEEK_SET)) goto done;
    stage=2; raw=malloc((size_t)length);
    if(!raw || fread(raw,1,(size_t)length,input)!=(size_t)length || !identity(raw,(DWORD)length)) goto done;
    stage=3; dos=(IMAGE_DOS_HEADER *)raw;
    if(dos->e_magic!=IMAGE_DOS_SIGNATURE || dos->e_lfanew<0 || !inside((size_t)dos->e_lfanew,sizeof(*pe),(size_t)length)) goto done;
    stage=4; pe=(IMAGE_NT_HEADERS32 *)(raw+dos->e_lfanew);
    if(pe->Signature!=IMAGE_NT_SIGNATURE || pe->FileHeader.Machine!=IMAGE_FILE_MACHINE_I386 ||
       pe->OptionalHeader.Magic!=IMAGE_NT_OPTIONAL_HDR32_MAGIC || pe->OptionalHeader.ImageBase!=0x400000 ||
       pe->OptionalHeader.SizeOfImage>32*1024*1024 || pe->OptionalHeader.SizeOfImage<0x700000 ||
       pe->FileHeader.NumberOfSections>96 || !inside(0,pe->OptionalHeader.SizeOfHeaders,(size_t)length)) goto done;
    stage=5; IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    if(!inside((size_t)((unsigned char *)sections-raw),pe->FileHeader.NumberOfSections*sizeof(*sections),(size_t)length)) goto done;
    stage=6; for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++) {
        if(!inside(sections[i].PointerToRawData,sections[i].SizeOfRawData,(size_t)length) ||
           !inside(sections[i].VirtualAddress,sections[i].SizeOfRawData,pe->OptionalHeader.SizeOfImage) ||
           !inside(sections[i].VirtualAddress,sections[i].Misc.VirtualSize,pe->OptionalHeader.SizeOfImage)) goto done;
    }
    stage=7; image=VirtualAlloc((void *)0x400000,pe->OptionalHeader.SizeOfImage,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!image) image=VirtualAlloc(NULL,pe->OptionalHeader.SizeOfImage,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!image) goto done;
    memcpy(image,raw,pe->OptionalHeader.SizeOfHeaders);
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++) {
        memcpy(image+sections[i].VirtualAddress,raw+sections[i].PointerToRawData,sections[i].SizeOfRawData);
    }
    if(image!=(void *)0x400000) {
        IMAGE_DATA_DIRECTORY reloc=pe->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_BASERELOC];
        if(!reloc.Size || !inside(reloc.VirtualAddress,reloc.Size,pe->OptionalHeader.SizeOfImage)) goto failed;
        size_t cursor=reloc.VirtualAddress,end=cursor+reloc.Size;
        while(cursor<end) {
            if(!inside(cursor,sizeof(IMAGE_BASE_RELOCATION),end)) goto failed;
            IMAGE_BASE_RELOCATION *block=(IMAGE_BASE_RELOCATION *)(image+cursor);
            if(block->SizeOfBlock<sizeof(*block) || (block->SizeOfBlock-sizeof(*block))%2 || !inside(cursor,block->SizeOfBlock,end)) goto failed;
            WORD *entries=(WORD *)(image+cursor+sizeof(*block));
            unsigned count=(block->SizeOfBlock-sizeof(*block))/2;
            for(unsigned i=0;i<count;i++) {
                unsigned type=entries[i]>>12;
                size_t target=(size_t)block->VirtualAddress+(entries[i]&0xfff);
                if(type==IMAGE_REL_BASED_ABSOLUTE) continue;
                if(type!=IMAGE_REL_BASED_HIGHLOW || !inside(target,4,pe->OptionalHeader.SizeOfImage)) goto failed;
                DWORD value;
                memcpy(&value,image+target,4);
                value+=(DWORD)(uintptr_t)image-0x400000;
                memcpy(image+target,&value,4);
            }
            cursor+=block->SizeOfBlock;
        }
    }
    stage=8; DWORD previous;
    if(!VirtualProtect(image,pe->OptionalHeader.SizeOfImage,PAGE_READONLY,&previous)) goto failed;
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++) {
        size_t size=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData ? sections[i].Misc.VirtualSize : sections[i].SizeOfRawData;
        if(size && (sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE) &&
           !VirtualProtect(image+sections[i].VirtualAddress,size,PAGE_EXECUTE_READ,&previous)) goto failed;
    }
    if(!FlushInstructionCache(GetCurrentProcess(),image,pe->OptionalHeader.SizeOfImage)) goto failed;
    goto done;
failed:
    VirtualFree(image,0,MEM_RELEASE); image=NULL;
done:
    if(!image) fprintf(stderr,"map_stage=%u win_error=%lu harness_base=%p\n",stage,GetLastError(),GetModuleHandleW(NULL));
    free(raw);
    fclose(input);
    return image;
}

#endif
