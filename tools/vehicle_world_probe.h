/* Whole unchanged manager/world calls share these owned single-body buffers. */
static unsigned vw_steps_completed,vw_contacts_produced,vw_empty_steps,vw_repeated_contacts;
static unsigned vw_finite(const void *p,unsigned n){for(unsigned i=0;i<n;i++)if(!isfinite(((const float *)p)[i]))return 0;return 1;}
static unsigned vw_body(unsigned address,unsigned char *p,unsigned *error){if(!address)return 0;if(address==(unsigned)(uintptr_t)p)return 1;*error=44;return 0;}
static unsigned vw_slot(unsigned address,WheelOwner *o,unsigned *error){if(!address)return 0;for(unsigned i=0;i<4;i++)if(address==(unsigned)(uintptr_t)o->slots[i])return i+1;*error=44;return 0;}

static unsigned vw_setup(WheelOwner *o,VehicleWorldRecord *r,unsigned char *p,unsigned char *image,unsigned delta){
 if(!r->steps||r->steps>64||r->parameters[0]!=delta||!delta||!vw_finite(r->parameters,11)||!vw_finite(r->material,13)||!vw_finite(r->cache+2,13)||!vw_finite(r->properties,17)||!vw_finite(r->workspace,8)||!vw_finite(r->rows,16*48)||!vw_finite(r->correction+1,4)||r->correction[0]>32||get(p,0x3b4)!=0x7f7fffff||p[0x415]||get(p,0x328)||get(o->producer,0x920)!=4||get(o->producer,0x924))return 44;
 for(unsigned i=0;i<11;i++)if(r->world[i])return 44;
 for(unsigned i=0;i<6;i++)if(r->lifecycle[i])return 44;
 for(unsigned i=0;i<16;i++)if(r->row_refs[i])return 44;
 for(unsigned i=0;i<4;i++)if(r->ends[i]>16)return 44;
 if(r->cache[0]!=r->cache[1])return 44;
 const unsigned links[]={0,4,8,0xc,0x10,0x14,0x18,0x1c,0x28,0x2f0,0x2f4,0x2fc,0x300,0x304,0x308,0x30c,0x318,0x320,0x324,0x400};
 for(unsigned i=0;i<sizeof(links)/sizeof(*links);i++)if(get(p,links[i]))return 44;
 if(p[0x2c]||p[0x2d]||p[0x412]||p[0x413])return 44;
 memcpy(o->rows,r->rows,sizeof(o->rows));memcpy(o->material,r->material,52);o->properties[0]=r->properties[0];memcpy(o->properties+4,r->properties+1,64);memcpy(o->workspace,r->workspace,32);
 o->manager_ptrs[0]=(unsigned)(uintptr_t)o->manager;o->worker_ptrs[0]=(unsigned)(uintptr_t)o->worker;o->update_lists[0]=(unsigned)(uintptr_t)o->update_list;o->update_list[1]=(unsigned)(uintptr_t)o->world;o->bodyptrs[0]=(unsigned)(uintptr_t)p;
 put(o->world,4,1);put(o->world,8,(unsigned)(uintptr_t)o->update_lists);put(o->world,0xc,1);put(o->world,0x10,(unsigned)(uintptr_t)o->manager_ptrs);put(o->world,0x3c,(unsigned)(uintptr_t)p);put(o->world,0x44,(unsigned)(uintptr_t)p);put(o->world,0x64,1);put(o->world,0x84,(unsigned)(uintptr_t)o->broadphase);
 put(o->producer,0xa80,(unsigned)(uintptr_t)o->world);put(o->producer,0x928,(unsigned)(uintptr_t)o->worker_ptrs);put(o->producer,0x92c,1);
 put(o->worker,0xc,(unsigned)(uintptr_t)o->worker_pairs);put(o->worker,0x18,(unsigned)(uintptr_t)o->worker_contacts);
 put(p,0x32c,(unsigned)(uintptr_t)o->world);put(o->car_shape,0xc8,(unsigned)(uintptr_t)o->material);
 for(unsigned i=0;i<wheel_source_count;i++){if(get(wheel_source_objects[i].shape,0xd8)||get(wheel_source_objects[i].shape,0x21c))return 44;put(wheel_source_objects[i].shape,0xc8,(unsigned)(uintptr_t)o->material);}
 unsigned char *m=o->manager;put(m,0,(unsigned)(uintptr_t)(image+0xd42654-0x400000));memcpy(m+0x10,r->parameters+1,16);memcpy(m+0x20,r->cache,60);
 for(unsigned i=0;i<2;i++){unsigned token=r->cache[i];if(token!=0xffffffff&&token!=0)return 44;put(m,0x20+4*i,token==0xffffffff?0:(unsigned)(uintptr_t)o->material);}
 put(m,0x68,16);put(m,0x6c,64);put(m,0x70,16);put(m,0x80,(unsigned)(uintptr_t)o->workspace);put(m,0x84,(unsigned)(uintptr_t)(o->properties+4));put(m,0x88,(unsigned)(uintptr_t)o->properties);put(m,0x8c,(unsigned)(uintptr_t)o->rows);put(m,0x90,(unsigned)(uintptr_t)o->bodyptrs);put(m,0x94,(unsigned)(uintptr_t)o->all_contacts);put(m,0x98,(unsigned)(uintptr_t)o->single_contacts);put(m,0xa0,(unsigned)(uintptr_t)o->two_contacts);
 put(m,0xc4,r->ends[0]);put(m,0xbc,r->ends[1]);put(m,0xc8,r->ends[2]);put(m,0xc0,r->ends[3]);memcpy(m+0xcc,r->parameters+5,16);put(m,0xdc,4);memcpy(m+0xe0,r->correction+1,12);memcpy(m+0xf0,r->correction+4,4);put(m,0xf4,r->correction[0]);memcpy(image+0xe52f24-0x400000,r->parameters+9,8);
 memcpy(o->world_before,o->world,sizeof(o->world));memcpy(o->producer_before,o->producer,sizeof(o->producer));memcpy(o->worker_before,o->worker,sizeof(o->worker));memcpy(o->manager_before,o->manager,sizeof(o->manager_before));return 0;
}

static unsigned vw_step(WheelOwner *o,const VehicleWorldRecord *r,unsigned char *p,unsigned char *image){
 unsigned used=get(o->producer,0x924);if(used>4||p[0x415])return 44;if(used>o->max_used)o->max_used=used;
 vw_contacts_produced+=used;vw_empty_steps+=used==0;if(get(o->world,0x7c8))vw_repeated_contacts+=used;
 ((void (__fastcall *)(void *,void *,unsigned))(image+0xad8d70-0x400000))(o->world,NULL,r->parameters[0]);
 ((void (__fastcall *)(void *,void *))(image+0xa727a0-0x400000))(o->world_wrapper,NULL);
 if(get(o->producer,0x924)||get(o->world,0x7c4)||get(o->world,0x7cc)||get(o->manager,0x74)!=(unsigned)(uintptr_t)p||get(p,0x30)||get(p,0x20)!=1||get(p,0x24)!=used)return 44;
 unsigned constructed=0,normals=0;
 for(unsigned contact=get(p,0),count=0;contact;contact=get((void *)(uintptr_t)contact,0x10)){
  unsigned error=0;if(++count>used||!vw_slot(contact,o,&error)||error)return 44;
  constructed+=get((void *)(uintptr_t)contact,0x14);normals+=get((void *)(uintptr_t)contact,0x18);
 }
 if(constructed>16||normals>constructed)return 44;
 for(unsigned i=0;i<constructed;i++)o->row_refs[i]=i>=normals?normals:0;
 ++vw_steps_completed;return 0;
}

static unsigned vw_output(WheelOwner *o,const unsigned char *before,DriverRecord *record,unsigned char *b,unsigned char *p){
 VehicleWorldRecord *r=&record->world;unsigned error=0;
 if(get(b,0xdd8)!=(unsigned)(uintptr_t)o->world_wrapper||get(b,0xdc8)!=(get(record->body,0xdc8)?(unsigned)(uintptr_t)o->car_wrapper:0u)||get(o->world_wrapper,4)!=(unsigned)(uintptr_t)o->world)return 44;
 if(memcmp(o->car_shape,before+offsetof(WheelOwner,car_shape),sizeof(o->car_shape))||memcmp(o->car_wrapper,before+offsetof(WheelOwner,car_wrapper),sizeof(o->car_wrapper))||memcmp(o->world_wrapper,before+offsetof(WheelOwner,world_wrapper),sizeof(o->world_wrapper))||memcmp(o->material,before+offsetof(WheelOwner,material),sizeof(o->material)))return 45;
 if(o->worker_ptrs[0]!=(unsigned)(uintptr_t)o->worker||o->manager_ptrs[0]!=(unsigned)(uintptr_t)o->manager||o->bodyptrs[0]!=(unsigned)(uintptr_t)p||o->update_lists[0]!=(unsigned)(uintptr_t)o->update_list||o->update_list[1]!=(unsigned)(uintptr_t)o->world||o->properties[1]||o->properties[2]||o->properties[3])return 46;
 if(o->update_list[0]!=(unsigned)(uintptr_t)p||get(p,0x318))return 49;
 for(unsigned i=0;i<wheel_source_count;i++)if(get(wheel_source_objects[i].shape,0xc8)!=(unsigned)(uintptr_t)o->material||get(wheel_source_objects[i].shape,0xd8)||get(wheel_source_objects[i].shape,0x10)!=11||get(wheel_source_objects[i].shape,0x21c))return 47;
 for(unsigned i=0;i<4;i++){unsigned ignored=vw_slot(o->all_contacts[i],o,&error);ignored=vw_slot(o->single_contacts[i],o,&error);(void)ignored;if(o->two_contacts[i])return 48;}
 const unsigned changed[]={0x38,0x3c,0x40,0x44,0x48,0x50,0x60,0x64,0x7c8,0x7f8,0x7fc};
 for(unsigned i=0;i<sizeof(changed)/sizeof(*changed);i++)memcpy(o->world_before+changed[i],o->world+changed[i],4);
 put(o->worker_before,0x1c,get(o->worker_before,0x18));put(o->worker_before,0x40,get(o->worker_before,0xc));
 if(memcmp(o->world_before,o->world,sizeof(o->world))||memcmp(o->producer_before,o->producer,sizeof(o->producer))||memcmp(o->worker_before,o->worker,sizeof(o->worker)))return 44;
 memcpy(o->manager_before+0x74,o->manager+0x74,4);
 for(unsigned offset=0xa8;offset<=0xb8;offset+=4)memcpy(o->manager_before+offset,o->manager+offset,4);
 if(memcmp(o->manager_before,o->manager,0x20)||memcmp(o->manager_before+0x5c,o->manager+0x5c,0x60)||memcmp(o->manager_before+0xcc,o->manager+0xcc,0x34))return 44;
 for(unsigned i=0x100;i<sizeof(o->manager);i++)if(i<0x118||i>=0x11c){if(o->manager[i])return 44;}
 for(unsigned i=0;i<sizeof(o->broadphase);i++)if(o->broadphase[i])return 44;
 for(unsigned i=0;i<128;i++)if(o->worker_contacts[i]||o->worker_pairs[i])return 44;
 memcpy(r->rows,o->rows,sizeof(r->rows));r->properties[0]=o->properties[0];memcpy(r->properties+1,o->properties+4,64);memcpy(r->workspace,o->workspace,sizeof(r->workspace));memcpy(r->cache,o->manager+0x20,sizeof(r->cache));memcpy(r->row_refs,o->row_refs,sizeof(r->row_refs));
 for(unsigned i=0;i<2;i++){unsigned address=r->cache[i];if(address&&address!=(unsigned)(uintptr_t)o->material)return 44;r->cache[i]=address?0:0xffffffff;}
 r->ends[0]=get(o->manager,0xc4);r->ends[1]=get(o->manager,0xbc);r->ends[2]=get(o->manager,0xc8);r->ends[3]=get(o->manager,0xc0);
 for(unsigned i=0;i<16;i++)if(o->row_refs[i]){unsigned address=r->rows[i][0xa8/4],offset=address-(unsigned)(uintptr_t)o->rows;if(offset%192!=0x98||offset/192>=o->row_refs[i])return 44;r->rows[i][0xa8/4]=offset/192;}
 const unsigned fields[]={0x50,0x74,0x7fc,0x7f8,0x38,0x60,0x64,0x3c,0x40,0x44,0x48};
 for(unsigned i=0;i<11;i++){unsigned value=get(i==1?o->manager:o->world,fields[i]);r->world[i]=(i==0||i==1||i==4||i>=7)?vw_body(value,p,&error):value;}
 r->lifecycle[0]=get(o->world,0x7c4);r->lifecycle[1]=get(o->world,0x7c8);r->lifecycle[2]=get(o->producer,0x924);r->lifecycle[3]=get(o->producer,0xa78);r->lifecycle[4]=get(o->producer,0xa7c);r->lifecycle[5]=get(o->world,0x7cc);
 const unsigned bodylinks[]={8,0xc,0x10,0x14,0x18,0x1c,0x2f0,0x2f4,0x2fc,0x300,0x304,0x318};
 for(unsigned i=0;i<sizeof(bodylinks)/sizeof(*bodylinks);i++)put(p,bodylinks[i],vw_body(get(p,bodylinks[i]),p,&error));
 for(unsigned i=0;i<2;i++)put(p,4*i,vw_slot(get(p,4*i),o,&error));
 for(unsigned i=0;i<o->max_used;i++){
  unsigned *c=o->slots[i];if(c[1]!=(unsigned)(uintptr_t)p||c[2]||c[0x28/4]!=(unsigned)(uintptr_t)o->car_shape||c[0x30/4]!=(unsigned)(uintptr_t)o->material||c[0x34/4]!=(unsigned)(uintptr_t)o->material)return 44;
  unsigned token=wheel_token(c[0x2c/4],0);if(token==0xffffffff)return 44;c[1]=1;c[0x28/4]=1;c[0x2c/4]=token;c[0x30/4]=0;c[0x34/4]=0;c[0x10/4]=vw_slot(c[0x10/4],o,&error);
 }
 memcpy(record->contact_slots,o->slots,sizeof(record->contact_slots));record->producer_used=0;put(b,0xdd8,0);put(b,0xdc8,get(record->body,0xdc8));return error;
}
