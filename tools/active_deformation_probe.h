/* Authored preflight for owned grids, shared by opt-in whole sync/job probes.
   Original execution remains intact. Zero dimensions retain the old domain. */
static unsigned grid_deformation_word(const unsigned char *v,unsigned o){unsigned w;memcpy(&w,v+o,4);return w;}
static float grid_deformation_float(const unsigned char *v,unsigned o){float f;memcpy(&f,v+o,4);return f;}
static int grid_deformation_guard(const unsigned char *v){
    unsigned nx=grid_deformation_word(v,0xc714),ny=grid_deformation_word(v,0xc718),nz=grid_deformation_word(v,0xc71c);
    if(!nx&&!ny&&!nz)return 1;
    if(nx<2||nx>16||ny<2||ny>16||nz<2||nz>16||nx*ny*nz>400)return 0;
    unsigned next=(grid_deformation_word(v,0xc7ec)+1)%10;
    float maximum=next==0?0.f:grid_deformation_float(v,0xc7c4);
    for(unsigned i=1;i<10;i++){float value=i==next?0.f:grid_deformation_float(v,0xc7c4+i*4);if(value>maximum)maximum=value;}
    if(maximum>=0.5f||!v[0xc7b1])return 1;
    for(unsigned i=0;i<nx*ny*nz;i++)for(unsigned lane=0;lane<8;lane++){
        float value=grid_deformation_float(v,0x4880+i*80+lane*4);
        if(!isfinite(value)||fabsf(value)>1000000.f)return 0;
    }
    return 1;
}
