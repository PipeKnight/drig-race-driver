//! Borrowed base-level BC1/BC3 blocks. Remaining mip layout stays opaque.
use crate::{Result, fail, pssg::Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bc1,
    Bc3,
}

#[derive(Debug)]
pub struct Texture<'a> {
    pub id: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub format: Format,
    /// Observed attribute counts levels following the base for square samples.
    pub declared_mip_levels: u32,
    pub base_blocks: &'a [u8],
    pub remaining_data: &'a [u8],
}

impl Texture<'_> {
    /// Use the full chain only when count and tightly packed BC sizes agree.
    /// Unknown padded layouts retain the previously verified base-only image.
    pub fn validated_dds(&self) -> (Vec<u8>, u32) {
        let mut width = self.width;
        let mut height = self.height;
        let mut size = 0usize;
        let mut levels = 0u32;
        loop {
            size += width.div_ceil(4) as usize
                * height.div_ceil(4) as usize
                * if self.format == Format::Bc1 { 8 } else { 16 };
            levels += 1;
            if width == 1 && height == 1 {
                break;
            }
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
        let mut dds = self.base_dds();
        if levels > 1
            && self.declared_mip_levels == levels - 1
            && self.base_blocks.len() + self.remaining_data.len() == size
        {
            for (offset, value) in [(8, 0x000a_1007u32), (28, levels), (108, 0x0040_1008)] {
                dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            dds.extend_from_slice(self.remaining_data);
            (dds, levels)
        } else {
            (dds, 1)
        }
    }
    /// Standard DDS envelope for only the validated base image, no mip tail.
    pub fn base_dds(&self) -> Vec<u8> {
        let mut bytes = vec![0; 128];
        bytes[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124u32),
            (8, 0x0008_1007),
            (12, self.height),
            (16, self.width),
            (20, self.base_blocks.len() as u32),
            (76, 32),
            (80, 4),
            (108, 0x1000),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[84..88].copy_from_slice(match self.format {
            Format::Bc1 => b"DXT1",
            Format::Bc3 => b"DXT5",
        });
        bytes.extend_from_slice(self.base_blocks);
        bytes
    }
}

pub fn read_textures<'a>(tree: &Tree<'a>) -> Result<Vec<Texture<'a>>> {
    let mut textures = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"TEXTURE" {
            continue;
        }
        let width = tree.integer(i, b"width")?;
        let height = tree.integer(i, b"height")?;
        if width == 0
            || height == 0
            || width > 16_384
            || height > 16_384
            || !width.is_multiple_of(4)
            || !height.is_multiple_of(4)
        {
            return fail(tree.nodes[i].offset, "unsupported texture dimensions");
        }
        let format = match tree.string(i, b"texelFormat")? {
            b"dxt1" => Format::Bc1,
            b"dxt5" => Format::Bc3,
            _ => return fail(tree.nodes[i].offset, "unsupported texture format"),
        };
        if tree.integer(i, b"imageBlockCount")? != 1 {
            return fail(tree.nodes[i].offset, "only one image block supported");
        }
        let blocks: Vec<_> = tree.nodes[i]
            .children
            .iter()
            .copied()
            .filter(|&j| tree.name(j) == b"TEXTUREIMAGEBLOCK")
            .collect();
        if blocks.len() != 1 {
            return fail(tree.nodes[i].offset, "texture image block count mismatch");
        }
        let block = blocks[0];
        if tree.string(block, b"typename")? != b"Raw" {
            return fail(
                tree.nodes[block].offset,
                "only Raw texture image blocks supported",
            );
        }
        let children = &tree.nodes[block].children;
        if children.len() != 1 || tree.name(children[0]) != b"TEXTUREIMAGEBLOCKDATA" {
            return fail(tree.nodes[block].offset, "expected one texture data child");
        }
        let data = tree.nodes[children[0]].data.unwrap();
        if data.len() != tree.integer(block, b"size")? as usize {
            return fail(tree.nodes[block].offset, "texture data size mismatch");
        }
        let base_size = (width as usize / 4)
            * (height as usize / 4)
            * if format == Format::Bc1 { 8 } else { 16 };
        if base_size > data.len() {
            return fail(tree.nodes[block].offset, "truncated base texture blocks");
        }
        textures.push(Texture {
            id: tree.string(i, b"id")?,
            width,
            height,
            format,
            declared_mip_levels: tree.integer(i, b"numberMipMapLevels")?,
            base_blocks: &data[..base_size],
            remaining_data: &data[base_size..],
        });
    }
    Ok(textures)
}
