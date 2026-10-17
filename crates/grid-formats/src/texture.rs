//! Borrowed BC1/BC3 blocks with count/size-validated tightly packed mip chains.
use crate::{Result, fail, pssg::Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bc1,
    Bc3,
    /// `ui8x4`: uncompressed 32-bit texels, B, G, R, A byte order.
    Bgra8,
}
impl Format {
    /// Bytes of a `width` x `height` level.
    pub fn level_size(self, width: u32, height: u32) -> usize {
        match self {
            Format::Bc1 => width.div_ceil(4) as usize * height.div_ceil(4) as usize * 8,
            Format::Bc3 => width.div_ceil(4) as usize * height.div_ceil(4) as usize * 16,
            Format::Bgra8 => width as usize * height as usize * 4,
        }
    }
}

#[derive(Debug)]
pub struct Texture<'a> {
    pub id: &'a [u8],
    pub width: u32,
    pub height: u32,
    pub format: Format,
    /// Observed attribute counts levels following the base, including partial chains.
    pub declared_mip_levels: u32,
    pub base_blocks: &'a [u8],
    pub remaining_data: &'a [u8],
}

impl Texture<'_> {
    /// Use declared full or partial chains only when tightly packed BC sizes agree.
    /// Full power-of-two rectangular BC chains may have a small, block-aligned
    /// all-zero allocation tail. Never interpret nonzero or interleaved padding.
    pub fn validated_dds(&self) -> (Vec<u8>, u32) {
        let mut dds = self.base_dds();
        let Some(target_levels) = self.declared_mip_levels.checked_add(1) else {
            return (dds, 1);
        };
        if !(2..=32).contains(&target_levels) {
            return (dds, 1);
        }
        let mut width = self.width;
        let mut height = self.height;
        let mut size = 0usize;
        let mut levels = 0u32;
        while levels < target_levels {
            size += self.format.level_size(width, height);
            levels += 1;
            if width == 1 && height == 1 {
                break;
            }
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
        let stored_size = self.base_blocks.len() + self.remaining_data.len();
        let used_tail = size.saturating_sub(self.base_blocks.len());
        let padded_full_bc = matches!(self.format, Format::Bc1 | Format::Bc3)
            && self.width != self.height
            && self.width.is_power_of_two()
            && self.height.is_power_of_two()
            && width == 1
            && height == 1
            && stored_size > size
            && stored_size - size <= 2048
            && (stored_size - size).is_multiple_of(match self.format {
                Format::Bc1 => 8,
                _ => 16,
            })
            && self
                .remaining_data
                .get(used_tail..)
                .is_some_and(|tail| tail.iter().all(|&b| b == 0));
        if levels == target_levels && (stored_size == size || padded_full_bc) {
            for (offset, value) in [(8, 0x000a_1007u32), (28, levels), (108, 0x0040_1008)] {
                dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            dds.extend_from_slice(&self.remaining_data[..used_tail]);
            if self.format == Format::Bgra8 {
                dds[8..12].copy_from_slice(&0x0002_100fu32.to_le_bytes());
            }
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
        match self.format {
            Format::Bc1 => bytes[84..88].copy_from_slice(b"DXT1"),
            Format::Bc3 => bytes[84..88].copy_from_slice(b"DXT5"),
            Format::Bgra8 => {
                // Uncompressed A8R8G8B8: pitch flag instead of linear size.
                for (offset, value) in [
                    (8, 0x0000_100fu32),
                    (20, self.width * 4),
                    (80, 0x41),
                    (88, 32),
                    (92, 0x00ff_0000),
                    (96, 0x0000_ff00),
                    (100, 0x0000_00ff),
                    (104, 0xff00_0000),
                ] {
                    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
        }
        bytes.extend_from_slice(self.base_blocks);
        bytes
    }
}

pub fn read_textures<'a>(tree: &Tree<'a>) -> Result<Vec<Texture<'a>>> {
    read_matching(tree, |_| true, false)
}

/// Only the TEXTURE nodes whose id is listed; others (any format) are skipped.
/// Uncompressed `ui8x4` texels are accepted here.
pub fn read_named<'a>(tree: &Tree<'a>, ids: &[&[u8]]) -> Result<Vec<Texture<'a>>> {
    read_matching(tree, |id| ids.contains(&id), true)
}

fn read_matching<'a>(
    tree: &Tree<'a>,
    wanted: impl Fn(&[u8]) -> bool,
    uncompressed: bool,
) -> Result<Vec<Texture<'a>>> {
    let mut textures = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"TEXTURE" || !wanted(tree.string(i, b"id")?) {
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
            b"ui8x4" if uncompressed => Format::Bgra8,
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
        let base_size = format.level_size(width, height);
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
