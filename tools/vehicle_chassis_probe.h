/* Whole original chassis-motion and its real matrix/cross dependencies. */
static float chassis_f(const unsigned char *b,unsigned o){float x;memcpy(&x,b+o,4);return x;}
static unsigned tail_guard(const unsigned char *b,float delta){
    if(!isfinite(delta)||delta<=0||delta>.1f||!isfinite(1.f/delta))return 50;
    const unsigned fields[]={0x13d0,0x13d4,0x1648};
    const unsigned ranges[][2]={{0x50,0x24c},{0xb20,0xb30},{0x1494,0x14d0},{0x15c0,0x15d0},{0x15e0,0x1610},{0x1630,0x1640}};
    for(unsigned i=0;i<3;i++)if(!isfinite(chassis_f(b,fields[i])))return 50;
    for(unsigned i=0;i<6;i++)for(unsigned o=ranges[i][0];o<ranges[i][1];o+=4)if(!isfinite(chassis_f(b,o)))return 50;
    for(unsigned i=0;i<4;i++){
        unsigned w=0x1990+i*0x150;const unsigned wheel_fields[]={0,8,0x40};
        for(unsigned j=0;j<3;j++)if(!isfinite(chassis_f(b,w+wheel_fields[j])))return 50;
    }
    if(chassis_f(b,0x14a8)==chassis_f(b,0x14ac)||chassis_f(b,0x14c0)==chassis_f(b,0x14c4)||
       chassis_f(b,0x1498)<0||chassis_f(b,0x149c)<0||
       !isfinite(chassis_f(b,0x1ae0)-chassis_f(b,0x1990))||chassis_f(b,0x1ae0)==chassis_f(b,0x1990)||
       !isfinite(chassis_f(b,0x1c38)-chassis_f(b,0x1998))||chassis_f(b,0x1c38)==chassis_f(b,0x1998))return 50;
    const unsigned phases[]={0x60,0x64},rates[]={0x14cc,0x14c8};
    for(unsigned i=0;i<2;i++){
        float x=(chassis_f(b,rates[i])*fabsf(delta))*20.f+chassis_f(b,phases[i]);
        if(x>60.f)x=x-60.f;
        if(!isfinite(x)||x<0||x>60.f)return 50;
    }
    return 0;
}
typedef void (__fastcall *ChassisStep)(void *,void *,float);
static unsigned tail_run(unsigned char *image,unsigned char *body,float delta){
    ((ChassisStep)(image+0x9be1e0-0x400000))(body,NULL,delta);
    for(unsigned o=0x10;o<0x68;o+=4)if(!isfinite(chassis_f(body,o)))return 51;
    return 0;
}
