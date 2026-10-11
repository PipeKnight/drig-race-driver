/* Actual selected early A03670 schedule with whole original dependencies. */
static float driver_f(const unsigned char *b,unsigned o){float x;memcpy(&x,b+o,4);return x;}
static void driver_set(unsigned char *b,unsigned o,float x){memcpy(b+o,&x,4);}
static unsigned driver_guard_group=0;
static unsigned driver_guard(const unsigned char *b){
    driver_guard_group=1;
    if(b[0x24a]||get(b,0x2278)||b[0xde4]||b[0xdfc]||(get(b,0x1598)!=1&&get(b,0x1598)!=2)||!get(b,0x196c)||get(b,0x196c)>9||!b[0x1970]||!b[0x1971]||get(b,0x3c0)>=5||get(b,0x2e0)>=10)return 50;
    const unsigned fields[]={0x1568,0xe1c,0xe20,0xe34,0x159c,0x2e4,0x1648,0x1f14,0x1f34,0x15a0,0x15a4,0x15a8,0x15ac,0x15b0,
        0x17a0,0x17a4,0x185c,0x187c,0x1890,0x1894,0x1f94,0x1fa0,0x110c,0x226c,0x1ee4,0x1ee8,0x1eec,0x1ef0,
        0xe08,0xe0c,0xe10,0xe14,0x1f04,0x1f08,0x1f18,0x1f1c,0x1f64,0x1f68,0x139c,0x21c4,0x2214,0x2268};
    for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++){driver_guard_group=fields[i];if(!isfinite(driver_f(b,fields[i])))return 50;}
    const unsigned ranges[][2]={{0x15c0,0x1620},{0x2ec,0x304},{0x1f4c,0x1f5c}};
    for(unsigned i=0;i<3;i++)for(unsigned o=ranges[i][0];o<ranges[i][1];o+=4){driver_guard_group=o;if(!isfinite(driver_f(b,o)))return 50;}
    const unsigned positive[]={0x15a0,0x15a4,0x15a8,0x15ac,0x15b0,0x17a0,0x17a4};
    for(unsigned i=0;i<7;i++){driver_guard_group=positive[i];if(driver_f(b,positive[i])<=0)return 50;}
    for(unsigned i=0;i<4;i++){
        const unsigned offsets[]={8,0x78,0x100,0x104,0x10c,0xe0,0xa0,0x9c,0x12c};
        for(unsigned j=0;j<9;j++){driver_guard_group=0x1990+i*0x150+offsets[j];if(!isfinite(driver_f(b,driver_guard_group)))return 50;}
        driver_guard_group=0x1990+i*0x150+0xe0;
        if(driver_f(b,0x1990+i*0x150+0xe0)<=0||driver_f(b,0x1990+i*0x150+0x100)<=0)return 50;
    }
    if(get(b,0x1598)==2&&!b[0x1640]){
        float ratio=driver_f(b,0x18f4+4*get(b,0x196c)),inertia=driver_f(b,0x17a0)/(ratio*ratio);
        if(!isfinite(ratio)||!isfinite(driver_f(b,0x17a8))||(ratio!=0&&!isfinite(inertia)))return 50;
    }
    driver_guard_group=2;
    if(driver_f(b,0x1ef0)==driver_f(b,0x1eec)||driver_f(b,0xe14)>=1||driver_f(b,0x2ec)<0||driver_f(b,0x2ec)>2||driver_f(b,0x2f0)<0||driver_f(b,0x2f0)>2)return 50;
    return 0;
}
static float driver_speed(unsigned char *b){
    float x=driver_f(b,0x15c0),y=driver_f(b,0x15c4),z=driver_f(b,0x15c8),w=driver_f(b,0x15cc);
    float square=(z*z+w*w)+(x*x+y*y);if(square==0)return 0;
    float seed=_mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(square))),product=seed*square;
    return (1.f-product*seed)*(product*.5f)+product;
}
typedef void (__fastcall *DriverThisStep)(void *,void *,float);
typedef void (__cdecl *DriverStep)(void *,float);
static unsigned driver_run(unsigned char *image,unsigned char *b,unsigned char *view,float delta){
    driver_set(b,0x1568,delta+driver_f(b,0x1568));
    if(b[0x1108]){
        driver_set(b,0x1884,1.f);
        if(get(b,0x196c)!=9){float sums[2]={0};unsigned counts[2]={0};
            for(unsigned i=0;i<4;i++){unsigned wheel=0x1990+i*0x150,group=driver_f(b,wheel+0x10c)>0;float value=driver_f(b,wheel+0x104)*driver_f(b,wheel+0xe0);sums[group]=value+sums[group];++counts[group];}
            if(counts[0]&&counts[1]&&sums[1]/(float)counts[1]<sums[0]/(float)counts[0]-.20000000298023224f)driver_set(b,0x1884,driver_f(b,0xe20));
        }
    }
    unsigned ring=get(b,0x3c0);driver_set(b,0x3ac+ring*4,delta);put(b,0x3c0,(ring+1)%5);memcpy(b+0xb20,b+0x15c0,32);
    ((DriverStep)(image+0x9b5e80-0x400000))(b,delta);
    ((void (__cdecl *)(void *))(image+0x9904c0-0x400000))(b);
    if(b[0x1640])put(b,0x1598,2);
    else if(get(b,0x1598)==2)((void (__cdecl *)(void *,unsigned))(image+0x98e1b0-0x400000))(b,1);
    driver_set(b,0x159c,driver_f(b,0x159c)+delta);driver_set(b,0x2e4,driver_f(b,0x2e4)+delta);memcpy(b+0x1fc0,b+0x15c0,16);
    float current=driver_speed(b);if(!isfinite(current))return 51;
    b[0x164c]=(unsigned char)(current>driver_f(b,0x1648)||driver_f(b,0xe20)<=driver_f(b,0xe1c));driver_set(b,0x1648,current);
    ring=get(b,0x2e0);driver_set(b,0x290+ring*4,current);driver_set(b,0x2b8+ring*4,current);put(b,0x2e0,(ring+1)%10);
    if(b[0x1f78])((DriverStep)(image+0x9b6200-0x400000))(b,delta);
    driver_set(view,0x2a8,0);driver_set(b,0x1f14,driver_f(b,0x1f14)+delta);
    float burn=driver_f(b,0x185c)*.800000011920929f+.20000000298023224f;if(driver_f(b,0x187c)<.30000001192092896f)burn=1.5f;
    if(b[0xb10]){float amount=((((driver_f(b,0x1894)/driver_f(b,0x17a4))+1.f)*.5f)*burn)*driver_f(b,0x1f94)*delta;driver_set(b,0x1fa0,driver_f(b,0x1fa0)-amount);}
    if(driver_f(b,0x1fa0)<0)driver_set(b,0x1fa0,0);
    ((DriverThisStep)(image+0x9969c0-0x400000))(b,NULL,delta);
    ((DriverThisStep)(image+0x9b9da0-0x400000))(b,NULL,delta);
    ((DriverThisStep)(image+0x9b9450-0x400000))(b,NULL,delta);
    ((DriverThisStep)(image+0x9e1290-0x400000))(b,NULL,delta);
    ((DriverThisStep)(image+0x9bf690-0x400000))(b+0x2ec,NULL,delta);
    ((DriverThisStep)(image+0x9bde30-0x400000))(b,NULL,delta);
    const unsigned final_ranges[][2]={{0x15c0,0x15e0},{0x2ec,0x304},{0x1f4c,0x1f5c}};
    for(unsigned i=0;i<3;i++)for(unsigned o=final_ranges[i][0];o<final_ranges[i][1];o+=4)if(!isfinite(driver_f(b,o)))return 51;
    const unsigned final_fields[]={0x1884,0x1f10,0x1fa0};
    for(unsigned i=0;i<sizeof(final_fields)/sizeof(*final_fields);i++)if(!isfinite(driver_f(b,final_fields[i])))return 51;
    for(unsigned i=0;i<4;i++)if(!isfinite(driver_f(b,0x1990+i*0x150+0x104)))return 51;
    for(unsigned o=0x298;o<0x2ac;o+=4)if(!isfinite(driver_f(view,o)))return 51;
    return 0;
}
