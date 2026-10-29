//! Actual main colour/depth attachment capability; independent of vehicle, event and AA algorithm.
use bevy::{prelude::*, render::{render_resource::{Extent3d, TextureFormat}, view::{ViewDepthTexture, ViewTarget}}};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Capability {
    pub format: TextureFormat,
    pub samples: u32,
    pub size: UVec2,
    pub hdr: bool,
}
#[derive(Clone, Copy)]
struct Shape {
    main_format: TextureFormat, main_samples:u32, main_extent:Extent3d,
    sampled:Option<(TextureFormat,u32,Extent3d)>,
    depth_format:TextureFormat, depth_samples:u32, depth_extent:Extent3d,
}
impl Capability {
    pub(super) fn strict(format:TextureFormat,samples:u32,size:UVec2)->Self {
        Self {format,samples,size,hdr:format==TextureFormat::Rgba16Float}
    }
    pub(crate) fn read(target:&ViewTarget,depth:&ViewDepthTexture,hdr:bool,samples:u32,size:UVec2)->Option<Self> {
        Self::from_shape(hdr,samples,size,Shape {
            main_format:target.main_texture_format(),main_samples:target.main_texture().sample_count(),
            main_extent:target.main_texture().size(),
            sampled:target.sampled_main_texture().map(|t|(t.format(),t.sample_count(),t.size())),
            depth_format:depth.texture.format(),depth_samples:depth.texture.sample_count(),depth_extent:depth.texture.size(),
        }).filter(|c| target.main_texture().format()==c.format)
    }
    pub(crate) fn matches(self,target:&ViewTarget,depth:&ViewDepthTexture)->bool {
        Self::read(target,depth,self.hdr,self.samples,self.size)==Some(self)
    }
    fn from_shape(hdr:bool,samples:u32,size:UVec2,s:Shape)->Option<Self> {
        let format_ok=if hdr {s.main_format==TextureFormat::Rgba16Float} else {
            matches!(s.main_format,TextureFormat::Rgba8UnormSrgb|TextureFormat::Bgra8UnormSrgb|TextureFormat::Rgba8Unorm|TextureFormat::Bgra8Unorm)
        };
        let extent=Extent3d {width:size.x,height:size.y,depth_or_array_layers:1};
        let sampled_ok=if samples==1 {s.sampled.is_none()} else {
            s.sampled==Some((s.main_format,samples,extent))
        };
        (size.x>0&&size.y>0&&matches!(samples,1|2|4|8)&&format_ok&&s.main_samples==1
            &&s.main_extent==extent&&sampled_ok&&s.depth_format==TextureFormat::Depth32Float
            &&s.depth_samples==samples&&s.depth_extent==extent)
            .then_some(Self {format:s.main_format,samples,size,hdr})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_depth_resolve_contract_covers_single_sample_and_rejects_crossed_attachments() {
        let size=UVec2::new(1280,800);let extent=Extent3d {width:1280,height:800,depth_or_array_layers:1};
        // These are CPU attachment contracts, NOT runtime/device compatibility claims.
        for (hdr,format) in [(false,TextureFormat::Rgba8UnormSrgb),(false,TextureFormat::Bgra8UnormSrgb),(true,TextureFormat::Rgba16Float)] {
            for samples in [1,2,4,8] {
                let shape=Shape {main_format:format,main_samples:1,main_extent:extent,
                    sampled:(samples>1).then_some((format,samples,extent)),depth_format:TextureFormat::Depth32Float,
                    depth_samples:samples,depth_extent:extent};
                let cap=Capability::from_shape(hdr,samples,size,shape).unwrap();
                assert_eq!(cap.samples,samples);assert_eq!(cap.format,format);
                let mut crossed=shape;crossed.depth_samples=if samples==1 {4}else{1};
                assert!(Capability::from_shape(hdr,samples,size,crossed).is_none());
                let mut crossed=shape;crossed.sampled=if samples==1 {Some((format,4,extent))}else{None};
                assert!(Capability::from_shape(hdr,samples,size,crossed).is_none());
                let mut crossed=shape;crossed.depth_extent.width-=1;
                assert!(Capability::from_shape(hdr,samples,size,crossed).is_none());
                assert!(Capability::from_shape(!hdr,samples,size,shape).is_none());
            }
        }
    }
}
