//! BP01, DF01 and CD01 from the user's own purchased `grid.exe` (P5, P6). A
//! port of `tools/extract_body_impact_parameters.py`,
//! `tools/extract_deformation_parameters.py` and
//! `tools/extract_contact_directions.py`: the same build check, the same
//! file-backed initial data at the same addresses, the same bytes. Read-only;
//! no code is executed or disassembled. The result is private and stays on
//! the user's machine (local/ or the app data folder), never in the repository.

/// SHA-256 of the supported Steam `grid.exe`.
pub const STEAM_SHA256: &str = "dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f";

const BP01_ADDRESSES: [u32; 6] = [0xe4fc30, 0xcb0f28, 0xcafa58, 0xcb870c, 0xd17b10, 0xcb0f1c];

/// CD01 recipe: the 44 startup sampling directions that C7DAE0 stores at
/// F5E1B0 (docs/CONTACT_DIRECTIONS.md). The initializer only moves scalars, so
/// each x/y/z lane is one `.rdata` word of the Steam file (0 = the zeroed
/// lane); w is always zero. Recovered from the analyzed image once; the Steam
/// code section that holds C7DAE0 is SteamStub-encrypted, its data is not.
const CD01_SOURCES: [[u32; 3]; 44] = [
    [0, 0xcafe50, 0xcb0948],
    [0xcafe50, 0, 0xcaf888],
    [0, 0xcb0948, 0],
    [0xcafe50, 0xcaf888, 0xcafe50],
    [0xcaf888, 0xcafe50, 0],
    [0xcb0948, 0, 0xcafe50],
    [0xd42778, 0xd42774, 0xd4277c],
    [0xd42780, 0xd42770, 0xd4277c],
    [0xd42768, 0xcafe50, 0xd4276c],
    [0xd42780, 0xd42760, 0xd42764],
    [0xd42778, 0xd4275c, 0xd4277c],
    [0xd42758, 0xd42770, 0xd42754],
    [0xd42750, 0xd42774, 0xd4274c],
    [0xd42750, 0xd4275c, 0xd4274c],
    [0xd42758, 0xd42760, 0xd4274c],
    [0xd4273c, 0, 0xd42740],
    [0xd42738, 0xd42744, 0xd42734],
    [0xd42748, 0xd42730, 0xd42734],
    [0xd42724, 0xd42728, 0xd4272c],
    [0xd42738, 0xd42720, 0xd42734],
    [0xd4271c, 0xcafe50, 0xd42718],
    [0xd42710, 0xcafe50, 0xd42714],
    [0xd42708, 0xd4270c, 0xd42718],
    [0xd42748, 0xd42704, 0xd42734],
    [0xd42708, 0xd42700, 0xd42718],
    [0xd42724, 0xd426fc, 0xd4272c],
    [0xd426f4, 0xd426f8, 0],
    [0xd426f4, 0xd426f0, 0],
    [0xd426e8, 0xd426ec, 0],
    [0xd426e4, 0xd426ec, 0],
    [0xd426e0, 0xd426f0, 0xcafe50],
    [0xd426e0, 0xd426f8, 0xcafe50],
    [0xd426e4, 0xd426dc, 0xcafe50],
    [0xd426e8, 0xd426dc, 0xcafe50],
    [0xd426d8, 0xd4270c, 0xd426d4],
    [0xd426cc, 0xd42728, 0xd426d0],
    [0xd426c8, 0, 0xd426d4],
    [0xd426cc, 0xd426fc, 0xd426d0],
    [0xd426d8, 0xd42700, 0xd426d4],
    [0xd426bc, 0, 0xd426c0],
    [0xd426c4, 0xd42730, 0xd426b8],
    [0xd426b4, 0xd42744, 0xd426b8],
    [0xd426b4, 0xd42720, 0xd426b8],
    [0xd426c4, 0xd42704, 0xd426b8],
];

struct Section {
    rva: u32,
    virtual_size: u32,
    raw_size: u32,
    raw_offset: u32,
}

struct Pe<'a> {
    raw: &'a [u8],
    image_base: u64,
    sections: Vec<Section>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("PE read outside file at {o:#x}"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .ok_or_else(|| format!("PE read outside file at {o:#x}"))
}

impl<'a> Pe<'a> {
    /// The headers `tools/inspect_installation.py` `inspect_pe` reads.
    fn parse(raw: &'a [u8]) -> Result<Self, String> {
        if raw.get(..2) != Some(b"MZ") {
            return Err("missing DOS signature".into());
        }
        let pe = u32_at(raw, 0x3c)? as usize;
        if raw.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err("missing PE signature".into());
        }
        let count = u16_at(raw, pe + 6)? as usize;
        let optional_size = u16_at(raw, pe + 20)? as usize;
        let optional = pe + 24;
        let magic = u16_at(raw, optional)?;
        if !matches!(magic, 0x10b | 0x20b) || !(1..=96).contains(&count) {
            return Err("unsupported PE optional header or section count".into());
        }
        let image_base = if magic == 0x20b {
            u32_at(raw, optional + 24)? as u64 | ((u32_at(raw, optional + 28)? as u64) << 32)
        } else {
            u32_at(raw, optional + 28)? as u64
        };
        let sections = (0..count)
            .map(|i| {
                let o = optional + optional_size + i * 40;
                Ok(Section {
                    virtual_size: u32_at(raw, o + 8)?,
                    rva: u32_at(raw, o + 12)?,
                    raw_size: u32_at(raw, o + 16)?,
                    raw_offset: u32_at(raw, o + 20)?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            raw,
            image_base,
            sections,
        })
    }

    fn relative(&self, address: u32, section: &Section) -> i64 {
        address as i64 - self.image_base as i64 - section.rva as i64
    }

    /// BP01 rule: the first section whose file-backed bytes hold all 4 bytes.
    fn file_word(&self, address: u32) -> Result<[u8; 4], String> {
        let (section, relative) = self
            .sections
            .iter()
            .map(|s| (s, self.relative(address, s)))
            .find(|&(s, r)| 0 <= r && r <= s.raw_size as i64 - 4)
            .ok_or_else(|| format!("{address:#x} has no file-backed section"))?;
        let at = section.raw_offset as usize + relative as usize;
        self.raw
            .get(at..at + 4)
            .map(|b| b.try_into().unwrap())
            .ok_or_else(|| format!("{address:#x} is outside the file"))
    }

    /// DF01 rule: the first section whose mapped extent holds `count` bytes;
    /// bytes past the file-backed part are the loader's zero fill.
    fn mapped(&self, address: u32, count: usize) -> Result<Vec<u8>, String> {
        let (section, relative) = self
            .sections
            .iter()
            .map(|s| (s, self.relative(address, s)))
            .find(|&(s, r)| 0 <= r && r <= s.virtual_size.max(s.raw_size) as i64 - count as i64)
            .ok_or_else(|| format!("{address:#x} has no mapped section"))?;
        let available = (section.raw_size as i64 - relative).clamp(0, count as i64) as usize;
        let at = section.raw_offset as usize + relative as usize;
        let mut out = self
            .raw
            .get(at..at + available)
            .ok_or_else(|| format!("{address:#x} is outside the file"))?
            .to_vec();
        out.resize(count, 0);
        Ok(out)
    }
}

fn checked(exe: &[u8]) -> Result<Pe<'_>, String> {
    if hex(&sha256(exe)) != STEAM_SHA256 {
        return Err(
            "grid.exe is not the supported Steam build (SHA-256 mismatch); BP01/DF01/CD01 addresses are build-specific"
                .into(),
        );
    }
    Pe::parse(exe)
}

/// BP01 (28 bytes): six file-backed initial scalars.
pub fn body_impact(exe: &[u8]) -> Result<Vec<u8>, String> {
    let pe = checked(exe)?;
    let mut out = b"BP01".to_vec();
    for address in BP01_ADDRESSES {
        out.extend(pe.file_word(address)?);
    }
    Ok(out)
}

/// DF01 (252 bytes): the deformation contact limit and bulk parameters.
pub fn deformation(exe: &[u8]) -> Result<Vec<u8>, String> {
    let pe = checked(exe)?;
    let word = |a| pe.mapped(a, 4);
    let flag = |a| -> Result<Vec<u8>, String> {
        Ok(u32::from(pe.mapped(a, 1)?[0]).to_le_bytes().to_vec())
    };
    let disabled = flag(0xe85fad)?;
    let table = pe.mapped(0xe4fc48, 64)?;
    let scale = word(0xe4fc30)?;
    let mut limit = Vec::new();
    for a in [0xcafbdc, 0xcb093c, 0xe4fc2c, 0xe4fc3c, 0xcb3ba8, 0xe51d40] {
        limit.extend(word(a)?);
    }
    limit.extend([&scale[..], &table, &disabled].concat());
    let mut bulk = Vec::new();
    for a in [0xe4fc2c, 0xe51d40, 0xe500c8, 0xe500d0, 0xe500d4, 0xe500cc] {
        bulk.extend(word(a)?);
    }
    bulk.extend(pe.mapped(0xe500a4, 20)?);
    for a in [0xe4fc88, 0xe4fc90, 0xe4fc94, 0xe4fc8c] {
        bulk.extend(word(a)?);
    }
    bulk.extend(flag(0xe500b8)?);
    for a in [0xe4fc38, 0xe500c0, 0xe500c4, 0xd1676c] {
        bulk.extend(word(a)?);
    }
    bulk.extend([&scale[..], &table, &disabled].concat());
    let out = [&b"DF01"[..], &limit, &bulk].concat();
    if out.len() != 252 {
        return Err("DF01 layout is not 252 bytes".into());
    }
    Ok(out)
}

/// CD01 (712 bytes): the 44 unit body-contact sampling directions.
pub fn contact_directions(exe: &[u8]) -> Result<Vec<u8>, String> {
    let pe = checked(exe)?;
    let mut out = b"CD01".to_vec();
    out.extend(44u32.to_le_bytes());
    for lanes in CD01_SOURCES {
        let mut v = [0f32; 3];
        for (value, address) in v.iter_mut().zip(lanes) {
            if address != 0 {
                let word = pe.file_word(address)?;
                out.extend(word);
                *value = f32::from_le_bytes(word);
            } else {
                out.extend([0; 4]);
            }
        }
        out.extend([0; 4]);
        if ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]) - 1.).abs() >= 0.001 {
            return Err("CD01 direction is not a unit vector".into());
        }
    }
    Ok(out)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// FIPS 180-4 SHA-256 (for the build check; avoids a dependency).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut tail = data[data.len() / 64 * 64..].to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend((data.len() as u64 * 8).to_be_bytes());
    for block in data[..data.len() / 64 * 64]
        .chunks(64)
        .chain(tail.chunks(64))
    {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes(word.try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0; 32];
    for (chunk, word) in out.chunks_mut(4).zip(h) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&sha256(&million)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// Synthetic PE: the section rules (first match, file word vs. mapped
    /// extent with zero fill) on a made-up image, not game data.
    #[test]
    fn section_rules_follow_the_python_readers() {
        let mut raw = vec![0u8; 0x400];
        raw[..2].copy_from_slice(b"MZ");
        raw[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        raw[0x80..0x84].copy_from_slice(b"PE\0\0");
        raw[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        raw[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
        raw[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        raw[0x98 + 28..0x98 + 32].copy_from_slice(&0x400000u32.to_le_bytes());
        let s = 0x98 + 0xe0;
        for (i, v) in [0x100u32, 0x1000, 0x10, 0x200].iter().enumerate() {
            raw[s + 8 + i * 4..s + 12 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        raw[0x20c..0x210].copy_from_slice(&[1, 2, 3, 4]);
        let pe = Pe::parse(&raw).unwrap();
        assert_eq!(pe.file_word(0x40100c).unwrap(), [1, 2, 3, 4]);
        assert!(pe.file_word(0x40100e).is_err());
        assert_eq!(pe.mapped(0x40100e, 4).unwrap(), [3, 4, 0, 0]);
        assert_eq!(pe.mapped(0x401080, 4).unwrap(), [0; 4]);
        assert!(pe.mapped(0x4010fe, 4).is_err());
        assert!(checked(&raw).is_err());
    }

    /// The Rust readers give the bytes the Python tools wrote (local/*.bin).
    #[test]
    #[ignore = "needs the purchased grid.exe and the private Python outputs"]
    fn derived_parameters_equal_the_python_output() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let exe = std::fs::read(root.join("grid.exe")).unwrap();
        let bp01 = body_impact(&exe).unwrap();
        let df01 = deformation(&exe).unwrap();
        let read = |name| std::fs::read(std::env::var_os(name).unwrap()).unwrap();
        assert_eq!(bp01, read("GRID_BODY_EVENT_PARAMETERS"));
        assert_eq!(df01, read("GRID_DEFORMATION_PARAMETERS"));
        crate::body_impact_parameters::read(&bp01).unwrap();
        crate::deformation_parameters::read(&df01).unwrap();
    }

    /// CD01 from the Steam file alone equals the file made from the unpacked
    /// image (tools/extract_contact_directions.py). Skips without the inputs.
    #[test]
    fn contact_directions_equal_the_prepared_file() {
        let Some(root) = std::env::var_os("GRID_GAME_ROOT") else {
            eprintln!("skipped: GRID_GAME_ROOT is not set");
            return;
        };
        let prepared = std::env::var_os("GRID_CONTACT_DIRECTIONS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../local/body-contact-directions.bin")
            });
        let (Ok(exe), Ok(prepared)) = (
            std::fs::read(std::path::Path::new(&root).join("grid.exe")),
            std::fs::read(&prepared),
        ) else {
            eprintln!("skipped: grid.exe or the prepared CD01 is absent");
            return;
        };
        let cd01 = contact_directions(&exe).unwrap();
        assert_eq!(cd01.len(), 712);
        assert_eq!(cd01, prepared);
    }

    /// Every recipe lane is an aligned `.rdata` word, never code.
    #[test]
    fn contact_direction_sources_are_rdata_words() {
        for address in CD01_SOURCES.iter().flatten().filter(|&&a| a != 0) {
            assert!((0xc89000..0xda59a4).contains(address) && address % 4 == 0);
        }
        assert_eq!(CD01_SOURCES.iter().flatten().filter(|&&a| a != 0).count(), 119);
    }
}
