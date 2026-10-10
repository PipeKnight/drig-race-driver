/* Whole original control wrapper, bounded owned pointers and inactive gear search. */
static unsigned controls_guard(const unsigned char *body){
    const unsigned offsets[]={0xe1c,0xe20,0xe34,0xe3c,0xe40,0x1378,0x1894,0x17a4,0x17a8,0x185c,0x1974,0x1978,0x1980,0x226c,0x2280,0x22b0,0x125c,0x1260,0x1358,0x1ef4,
        0xe30,0x2294,0x1648,0x159c,0xe54,0x21d4,0x21d8,0x2298,0x229c,
        0xfdc,0x138c,0x123c,0x1240,0x1244,0xe70,0xe5c,0xe58,0x101c,0x1020,0x1024,0x1028,0xfec,0xfe8,0x1248,0x1250,0x124c,0x1254,0x1388,0x1f2c};
    if(body[0x1348]||get(body,0x196c)>9||get(body,0x21ec)>4||get(body,0x18c0)>4||get(body,0xe48)>10||(int)get(body,0xe2c)<-1||(int)get(body,0xe2c)>9)return 40;
    for(unsigned i=0;i<sizeof(offsets)/sizeof(*offsets);i++)if(!isfinite(*(float *)(body+offsets[i])))return 40;
    if(*(float *)(body+0x17a4)<=0||*(float *)(body+0x125c)+ *(float *)(body+0x1260)==0||*(float *)(body+0x21d8)<=0)return 40;
    const unsigned nonnegative[]={0xe54,0x21d4,0x2298,0x229c};
    for(unsigned i=0;i<4;i++)if(*(float *)(body+nonnegative[i])<0)return 40;
    if(body[0xe6c]&&(*(float *)(body+0xfdc)<0||*(float *)(body+0xfdc)>1||*(float *)(body+0x1f2c)<0||(*(float *)(body+0x123c)!=0&&*(float *)(body+0x1244)< *(float *)(body+0x1240))||(*(float *)(body+0x1024)>0&&*(float *)(body+0x1024)== *(float *)(body+0x1028))||(*(float *)(body+0x1248)!=0&&*(float *)(body+0x1250)!=0&&*(float *)(body+0x1254)< *(float *)(body+0x124c))))return 40;
    for(unsigned i=0;i<12;i++)if(!isfinite(*(float *)(body+0x18f4+i*4))||(i>0&&i<=9&&*(float *)(body+0x18f4+i*4)==0))return 40;
    for(unsigned i=0;i<10;i++)if(!isfinite(*(float *)(body+0x191c+i*4))||!isfinite(*(float *)(body+0x1944+i*4)))return 40;
    for(unsigned i=0;i<4;i++){
        const unsigned offsets_w[]={0,0x20,0x24,0x28,0x2c,0x38,0x78,0xa0,0xa4,0xe0,0xe8,0xf0,0xf4,0xf8,0x104,0x108,0x10c,0x144};
        for(unsigned j=0;j<sizeof(offsets_w)/sizeof(*offsets_w);j++)if(!isfinite(*(float *)(body+0x1990+i*0x150+offsets_w[j])))return 40;
        if(*(float *)(body+0x1990+i*0x150+0xf4)==0||*(float *)(body+0x1990+i*0x150+0xf0)<0||*(float *)(body+0x1990+i*0x150+0xf0)>1||!isfinite(*(float *)(body+0x2284+i*4))||!isfinite(*(float *)(body+0x22a0+i*4)))return 40;
    }
    for(unsigned i=0;i<32;i++)if(!isfinite(*(float *)(body+0x15c0+i*4)))return 40;
    return 0;
}
