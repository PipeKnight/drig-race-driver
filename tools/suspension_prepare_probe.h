/* Bounded whole 9E41F0 adapter, called after the real packed world is installed.
   Address-looking input words are opaque; consumed pointers must be zero and
   are relocated only to owned allocations. No helper replacements/code writes. */
typedef void (__fastcall *SuspensionPrepare)(void *,void *,float);
#ifdef GRID_VEHICLE_GRIP
typedef void (__cdecl *VehicleGrip)(void *,float);
#endif
static unsigned run_suspension_prepare(unsigned char *image,const wchar_t *request,const wchar_t *output,
    SurfaceRecord *materials,unsigned material_count,unsigned *completed){
    unsigned error=0,protected=0,number=0,tag=0;
    FILE *input=NULL,*result=NULL;
    unsigned char *saved=malloc(0x4000);if(!saved)return 60;
    const unsigned pages[]={0xe58000,0xe5a000,0xe5b000,0xe5e000};DWORD protections[4];
    IMAGE_NT_HEADERS32 *pe=(IMAGE_NT_HEADERS32 *)(image+((IMAGE_DOS_HEADER *)image)->e_lfanew);
    IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(pe);
    unsigned char *code=malloc(pe->OptionalHeader.SizeOfImage);
    if(!code){free(saved);return 60;}memcpy(code,image,pe->OptionalHeader.SizeOfImage);
    for(unsigned i=0;i<4;i++){
        unsigned char *page=image+pages[i]-0x400000;
        memcpy(saved+i*4096,page,4096);
        if(!VirtualProtect(page,4096,PAGE_READWRITE,protections+i)){error=61;goto finish;}
        ++protected;
    }
    if(!materials||!material_count||_wfopen_s(&input,request,L"rb")||!input){error=62;goto finish;}
    if(fread(&tag,4,1,input)!=1||(tag!=0x31305053&&tag!=0x31304653
#ifdef GRID_VEHICLE_GRIP
        &&tag!=0x31304756
#ifdef GRID_VEHICLE_VECTORS
        &&tag!=0x31305556
#endif
#endif
        )||fread(&number,4,1,input)!=1||!number||number>128){error=63;goto finish;}
    unsigned grip=0;
#ifdef GRID_VEHICLE_GRIP
    grip=tag==0x31304756;
#ifdef GRID_VEHICLE_VECTORS
    grip=grip||tag==0x31305556;
#endif
#endif
    const unsigned whole=tag==0x31304653||grip,record_size=grip?0x2908:(whole?0x2614:0x2608);
    unsigned char *records=malloc((size_t)number*record_size);
    if(!records){error=64;goto finish;}
    if(fread(records,record_size,number,input)!=number||fgetc(input)!=EOF||ferror(input)){free(records);error=65;goto finish;}
    for(unsigned index=0;index<number;index++){
        unsigned char *record=records+index*record_size;
        __declspec(align(16)) unsigned char body[0x2600];memcpy(body,record+8,sizeof(body));
        unsigned char *context=body+0x3d0;float delta;memcpy(&delta,record+4,4);
        if(getword(record)!=index||!isfinite(delta)||delta<=0||delta>.1f||!isfinite(1.f/delta)||getword(context+0x5d8)!=4
            ||context[0x70c]||getword(context+0x604)||getword(context+0x5d4)||getword(context+0x608)||getword(context+0x60c)){error=66;break;}
        const unsigned finite_blocks[][2]={{0xb20,32},{0x15a0,1},{0x15a8,3},{0x15c0,24},{0x1650,16},{0xac0,3},{0x159c,1},{0x1648,1},{0x12ac,1}};
        for(unsigned i=0;i<sizeof(finite_blocks)/sizeof(*finite_blocks);i++)for(unsigned j=0;j<finite_blocks[i][1];j++){
            float value;memcpy(&value,body+finite_blocks[i][0]+j*4,4);if(!isfinite(value)){error=67;break;}
        }
        if(*(float *)(body+0x15a0)<=0)error=67;
        for(unsigned j=0;j<3;j++)if(*(float *)(context+0x6f0+j*4)<=0)error=67;
        if(whole){
            if(getword(body)||!isfinite(*(float *)context)||*(float *)context<=0)error=67;
            if(getword(body+0x2278))error=67;
            for(unsigned j=0;j<3;j++)if(*(float *)(body+0x15a8+j*4)<=0)error=67;
            const unsigned extras[]={0x11e8,0x185c,0x2198,0x219c,0x21a0,0x21a4,0x21a8,0x21ac,0x21b0,0x21b4,0x21b8,0x21bc,0x21c0};
            for(unsigned j=0;j<sizeof(extras)/sizeof(*extras);j++)if(!isfinite(*(float *)(body+extras[j])))error=67;
            if(!(*(float *)(body+0x21b4)<*(float *)(body+0x21b8)&&*(float *)(body+0x21b8)<*(float *)(body+0x21bc)))error=67;
        }
        for(unsigned i=0;i<4;i++){
            unsigned char *c=context+i*0x170,*w=body+0x1990+i*0x150;
            if(getword(c+0x178)||getword(c+0x17c)){error=68;break;}
            const unsigned offsets[]={0,4,8,12,0x3c,0x40,0x48,0x4c,0x5c,0xe0,0x110,0x140};
            for(unsigned j=0;j<sizeof(offsets)/sizeof(*offsets);j++){float v;memcpy(&v,w+offsets[j],4);if(!isfinite(v))error=69;}
            if(*(float *)(w+0xe0)<=0||*(float *)(w+0x48)>*(float *)(w+0x4c))error=69;
            for(unsigned j=0;j<4;j++)if(!isfinite(*(float *)(c+0x50+j*4)))error=69;
            const unsigned history[]={0x6c,0x7c,0x80};
            for(unsigned j=0;j<3;j++)if(!isfinite(*(float *)(c+history[j])))error=69;
            if(whole){
                const unsigned parameters[]={0x50,0x54,0x58,0x60,0x64,0x68,0x70,0x114};
                for(unsigned j=0;j<8;j++)if(!isfinite(*(float *)(w+parameters[j])))error=69;
                if(*(float *)(w+0x4c)<=0||*(float *)(w+0x48)>*(float *)(w+0x50)||fabsf(*(float *)(w+0x48))>=99||fabsf(*(float *)(w+0x50))>=99)error=69;
                if(*(float *)(w+0x64)<0)error=69;
            }
            for(unsigned j=0;j<5;j++){
                float angle=*(float *)(body+0x30c+i*40+j*8),radius=*(float *)(body+0x310+i*40+j*8);
                if(!isfinite(angle)||!isfinite(radius)||radius<=0)error=69;
            }
            *(unsigned *)(c+0x178)=(unsigned)(uintptr_t)w;
        }
        if(error)break;
#ifdef GRID_VEHICLE_GRIP
        if(grip){
            const unsigned scalars[]={0xfcc,0xfe4,0xe1c,0xe20,0xe74,0x1160,0x1164,0x1168,0x116c,0x1170,0x1174,0x11f4,0x12a8,0x1354,0x137c,0x187c,0x1efc,0x1f10,0x1f20,0x1f24,0x1f3c,0x1f64,0x1f8c,0x1f90,0x1fa4,0x1fa8,0x1ffc,0x21f8,0x21fc,0x2200};
            for(unsigned j=0;j<sizeof(scalars)/sizeof(*scalars);j++)if(!isfinite(*(float *)(body+scalars[j])))error=78;
            for(unsigned j=0;j<10;j++)if(!isfinite(*(float *)(body+0x11fc+j*4)))error=78;
            if(getword(body+0x1620)!=4||getword(body+0x21ec)>4||*(float *)(body+0x1648)<0||*(float *)(body+0x1648)>1000000||*(float *)(body+0x1f3c)<=0||*(float *)(body+0x1ffc)<0||*(float *)(body+0x137c)<0||*(float *)(body+0x137c)>=1||*(float *)(body+0xfcc)<=0||*(float *)(body+0xfcc)>=1||*(float *)(body+0x1160)==0||*(float *)(body+0x21f8)==*(float *)(body+0x21fc)||*(float *)(body+0x12a8)<0||*(float *)(body+0x12a8)>4)error=78;
            for(unsigned i=0;i<4;i++){
                unsigned char *w=body+0x1990+i*0x150;const unsigned offsets[]={0xb0,0xb4,0xb8,0xbc,0x80,0x104,0xe4,0xec,0x10c,0x11c,0x120,0x124};
                for(unsigned j=0;j<12;j++)if(!isfinite(*(float *)(w+offsets[j])))error=78;
                unsigned token=getword(w+0x128);if(token>material_count)error=78;else if(token)*(unsigned *)(w+0x128)=(unsigned)(uintptr_t)materials[token-1].data;
#ifdef GRID_VEHICLE_VECTORS
                if(tag==0x31305556){
                    if(!isfinite(*(float *)(w+0x20))||!isfinite(*(float *)(w+0x24)))error=78;
                }else
#endif
                {
                const unsigned vectors[]={0x190,0x1d0,0x210};
                for(unsigned j=0;j<3;j++)for(unsigned k=0;k<4;k++)if(!isfinite(*(float *)(record+0x2608+vectors[j]+i*16+k*4)))error=78;
                }
                if(!isfinite(*(float *)(record+0x2608+0x298+i*4)))error=78;
            }
        }
        if(error)break;
#endif
        __declspec(align(16)) unsigned char table[0x20]={0};unsigned entry[3]={0};
        *(unsigned *)(table+0xc)=1;*(unsigned *)(table+0x14)=(unsigned)(uintptr_t)entry;
        *(unsigned *)(context+0x608)=(unsigned)(uintptr_t)table;
        *(unsigned *)(context+0x5d4)=(unsigned)(uintptr_t)body;
        __declspec(align(16)) unsigned char view[0x300]={0};
        if(grip)memcpy(view,record+0x2608,0x300);
        if(whole)*(unsigned *)body=(unsigned)(uintptr_t)view;
        *(unsigned *)(image+0xec2c54-0x400000)=0;
        __try {
#ifdef GRID_VEHICLE_VECTORS
            if(tag==0x31305556){
                unsigned owners[4];
                for(unsigned i=0;i<4;i++){
                    unsigned char *w=body+0x1990+i*0x150;owners[i]=getword(w+0x34);
                    *(unsigned *)(w+0x34)=(unsigned)(uintptr_t)(body+0x15a0);
                }
                ((void (__cdecl *)(void *))(image+0x9b2790-0x400000))(body);
                for(unsigned i=0;i<4;i++)*(unsigned *)(body+0x1990+i*0x150+0x34)=owners[i];
                float scale=*(float *)(body+0x3d0),amount=delta*.25f;
                *(float *)(body+0x3d0)=scale<1.f?fminf(scale+amount,1.f):fmaxf(scale-amount,1.f);
            }
#endif
#ifdef GRID_VEHICLE_GRIP
            if(grip)((VehicleGrip)(image+0xa00e10-0x400000))(body,delta);else
#endif
            ((SuspensionPrepare)(image+(whole?0x9f1e10:0x9e41f0)-0x400000))(context,NULL,delta);
        }
        __except(EXCEPTION_EXECUTE_HANDLER){fprintf(stderr,"case=%u fault=%08lx\n",index,GetExceptionCode());error=70;}
        if(error)break;
        if(whole){
            if(getword(body)!=(unsigned)(uintptr_t)view){error=71;break;}
            *(unsigned *)body=0;if(grip)memcpy(record+0x2608,view,0x300);else memcpy(record+0x2608,view+0x48,12);
        }
        *(unsigned *)(context+0x608)=0;*(unsigned *)(context+0x5d4)=0;
        for(unsigned i=0;i<4;i++){
            unsigned char *c=context+i*0x170,*w=body+0x1990+i*0x150;
            if(getword(c+0x178)!=(unsigned)(uintptr_t)w){error=71;break;}
            *(unsigned *)(c+0x178)=0;
            unsigned token=getword(w+0xc0);uintptr_t base=(uintptr_t)materials;
            if(token){
                if(token<base||token>=base+material_count*sizeof(SurfaceRecord)||(token-base)%sizeof(SurfaceRecord)){error=72;break;}
                *(unsigned *)(w+0xc0)=(unsigned)((token-base)/sizeof(SurfaceRecord))+1;
            }
            *(unsigned *)(w+0xc4)=getword(w+0xc4)!=0;*(unsigned *)(w+0xc8)=getword(w+0xc8)!=0;
            if(grip){
                unsigned history=getword(w+0x128);
                if(history){if(history<base||history>=base+material_count*sizeof(SurfaceRecord)||(history-base)%sizeof(SurfaceRecord)){error=79;break;}*(unsigned *)(w+0x128)=(unsigned)((history-base)/sizeof(SurfaceRecord))+1;}
                if(!isfinite(*(float *)(w+0x78))||!isfinite(*(float *)(view+0x5c+i*4))||!isfinite(*(float *)(view+0x6c+i*4))){error=80;break;}
            }
        }
        if(error)break;
        memcpy(record+8,body,sizeof(body));++*completed;
    }
    if(!error){
        HANDLE handle=CreateFileW(output,GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);
        if(handle==INVALID_HANDLE_VALUE)error=73;
        else {int fd=_open_osfhandle((intptr_t)handle,_O_BINARY);
            if(fd<0){CloseHandle(handle);error=73;}else{result=_fdopen(fd,"wb");if(!result){_close(fd);error=73;}}
        }
        if(!error&&(fwrite(&tag,4,1,result)!=1||fwrite(&number,4,1,result)!=1||fwrite(records,record_size,number,result)!=number))error=74;
    }
    free(records);
finish:
    for(unsigned i=0;i<protected;i++){
        unsigned char *page=image+pages[i]-0x400000;memcpy(page,saved+i*4096,4096);
        DWORD ignored;if(!VirtualProtect(page,4096,protections[i],&ignored))error=75;
    }
    for(unsigned i=0;i<pe->FileHeader.NumberOfSections;i++)if(sections[i].Characteristics&IMAGE_SCN_MEM_EXECUTE){
        size_t length=sections[i].Misc.VirtualSize>sections[i].SizeOfRawData?sections[i].Misc.VirtualSize:sections[i].SizeOfRawData;
        if(memcmp(code+sections[i].VirtualAddress,image+sections[i].VirtualAddress,length))error=76;
    }
    if(input&&fclose(input))error=77;if(result&&fclose(result))error=77;
    free(code);free(saved);printf("suspension_preparation_records=%u error=%u code_unchanged=%u\n",*completed,error,error!=76);return error;
}
