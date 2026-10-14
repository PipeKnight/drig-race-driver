//! Bounded installed DIC1/2.00 bank index. Unknown flags stay opaque.
//! Selected WIP/zero-flag samples expose PCM16 mono; other layouts are guarded.
use crate::{Result, fail, range};
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct Dictionary<'a> {
    pub opaque_id: u32,
    pub banks: Vec<Bank<'a>>,
}
#[derive(Debug)]
pub struct Bank<'a> {
    pub name: &'a str,
    pub extension: &'a str,
    pub data_bytes: usize,
    pub samples: Vec<Sample<'a>>,
}
#[derive(Debug)]
pub struct Sample<'a> {
    pub name: &'a str,
    pub offset: usize,
    pub end: usize,
    pub rate: u16,
    pub flags: u16,
}
fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes[range(offset, 4, bytes.len())?].try_into().unwrap(),
    ))
}
fn name(bytes: &[u8], offset: usize, width: usize) -> Result<&str> {
    let raw = &bytes[range(offset, width, bytes.len())?];
    let end = raw.iter().position(|&v| v == 0).unwrap_or(width);
    if end == 0
        || raw[end..].iter().any(|&v| v != 0)
        || !raw[..end]
            .iter()
            .all(|v| v.is_ascii_alphanumeric() || b"_-".contains(v))
    {
        return fail(offset, "invalid audio dictionary name");
    }
    std::str::from_utf8(&raw[..end]).map_err(|_| crate::Error {
        offset,
        message: "invalid audio dictionary ASCII",
    })
}
pub fn read(bytes: &[u8]) -> Result<Dictionary<'_>> {
    if !(16..=16 * 1024 * 1024).contains(&bytes.len())
        || &bytes[..4] != b"DIC1"
        || &bytes[8..12] != b"2.00"
    {
        return fail(0, "unsupported audio dictionary framing");
    }
    let count = word(bytes, 12)? as usize;
    if !(1..=4096).contains(&count) {
        return fail(12, "invalid audio bank count");
    }
    let mut cursor = range(16, count * 24, bytes.len())?.end;
    let mut banks = Vec::with_capacity(count);
    let mut names = BTreeSet::new();
    let mut total = 0usize;
    for i in 0..count {
        let at = 16 + i * 24;
        let table = word(bytes, at)? as usize;
        let n = word(bytes, at + 4)? as usize;
        let bank_name = name(bytes, at + 8, 16)?;
        if table != cursor
            || !(1..=65536).contains(&n)
            || !names.insert(bank_name)
            || total + n > 1_000_000
        {
            return fail(at, "unsupported audio dictionary table ownership");
        }
        let end = range(table, n * 24 + 8, bytes.len())?.end;
        let data_bytes = word(bytes, table + n * 24)? as usize;
        if !(1..=512 * 1024 * 1024).contains(&data_bytes) || word(bytes, table)? != 0 {
            return fail(table, "invalid audio bank extent");
        }
        let extension = name(bytes, table + n * 24 + 4, 4)?;
        let mut samples = Vec::with_capacity(n);
        let mut sample_names = BTreeSet::new();
        for j in 0..n {
            let at = table + j * 24;
            let offset = word(bytes, at)? as usize;
            let end = word(bytes, at + 24)? as usize;
            let format = word(bytes, at + 4)?;
            let sample_name = name(bytes, at + 8, 16)?;
            if offset >= end
                || end > data_bytes
                || format & 0xffff == 0
                || !sample_names.insert(sample_name)
            {
                return fail(at, "invalid audio sample extent or metadata");
            }
            samples.push(Sample {
                name: sample_name,
                offset,
                end,
                rate: format as u16,
                flags: (format >> 16) as u16,
            });
        }
        banks.push(Bank {
            name: bank_name,
            extension,
            data_bytes,
            samples,
        });
        total += n;
        cursor = end;
    }
    if cursor != bytes.len() {
        return fail(cursor, "trailing audio dictionary data");
    }
    Ok(Dictionary {
        opaque_id: word(bytes, 4)?,
        banks,
    })
}
impl Bank<'_> {
    /// Strict selected raw layout. Do not guess channels from unsupported flags
    /// or reinterpret WIM/WIO data as PCM. Complete bank extent must agree.
    pub fn mono_pcm16<'b>(&self, index: usize, data: &'b [u8]) -> Result<&'b [u8]> {
        let sample = self.samples.get(index).ok_or(crate::Error {
            offset: index,
            message: "invalid audio sample index",
        })?;
        if self.extension != "WIP"
            || sample.flags != 0
            || data.len() != self.data_bytes
            || sample.offset % 2 != 0
            || sample.end % 2 != 0
        {
            return fail(sample.offset, "unsupported selected mono PCM16 bank");
        }
        let count = sample.end.checked_sub(sample.offset).ok_or(crate::Error {
            offset: sample.offset,
            message: "invalid audio sample extent",
        })?;
        let record = &data[range(sample.offset, count, data.len())?];
        if record.len() < 6 {
            return fail(sample.offset, "PCM16 sample lacks prefix or payload");
        }
        // A29820/A29330 skip one opaque DWORD before WIP PCM16 publication.
        // It is not two audio samples. Retain it in the original bank bytes.
        Ok(&record[4..])
    }
    /// A standard independently authored WAV wrapper over exact local bytes;
    /// The native four-byte non-PCM prefix is skipped; no PCM resampling, gain,
    /// trimming or original mixing behavior is implied.
    pub fn mono_pcm16_wav(&self, index: usize, data: &[u8]) -> Result<Vec<u8>> {
        let pcm = self.mono_pcm16(index, data)?;
        let rate = u32::from(self.samples[index].rate);
        let mut wav = Vec::with_capacity(44 + pcm.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(pcm);
        Ok(wav)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![0; 96];
        b[..4].copy_from_slice(b"DIC1");
        b[8..12].copy_from_slice(b"2.00");
        for (at, value) in [
            (12, 1u32),
            (16, 40),
            (20, 2),
            (40, 0),
            (44, 32000),
            (64, 8),
            (68, 32000),
            (88, 16),
        ] {
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        b[24..28].copy_from_slice(b"bank");
        b[48..51].copy_from_slice(b"one");
        b[72..75].copy_from_slice(b"two");
        b[92..95].copy_from_slice(b"WIP");
        b
    }
    #[test]
    fn source_boundaries_preserve_every_pcm_word_and_guard_other_formats() {
        let bytes = fixture();
        let dict = read(&bytes).unwrap();
        let bank = &dict.banks[0];
        let data = [
            255, 255, 255, 255, 0, 128, 255, 127, 0x78, 0x56, 0x34, 0x12, 255, 255, 0, 0,
        ];
        assert_eq!(bank.samples[0].rate, 32000);
        assert_eq!(bank.mono_pcm16(0, &data).unwrap(), &data[4..8]);
        let wav = bank.mono_pcm16_wav(1, &data).unwrap();
        assert_eq!(&wav[44..], &data[12..]);
        assert_eq!(wav.len(), 48);
        assert_eq!(&wav[..4], b"RIFF");
        assert!(bank.mono_pcm16(2, &data).is_err());
        assert!(bank.mono_pcm16(0, &data[..6]).is_err());
        let mut unknown = fixture();
        unknown[46] = 0x10;
        let dict = read(&unknown).unwrap();
        assert_eq!(dict.banks[0].samples[0].flags, 0x10);
        assert!(dict.banks[0].mono_pcm16(0, &data).is_err());
        unknown[92..95].copy_from_slice(b"WIM");
        let dict = read(&unknown).unwrap();
        assert!(dict.banks[0].mono_pcm16(1, &data).is_err());
        let mut prefix_only = fixture();
        prefix_only[64..68].copy_from_slice(&4_u32.to_le_bytes());
        let dict = read(&prefix_only).unwrap();
        assert!(dict.banks[0].mono_pcm16(0, &data).is_err());
    }
    #[test]
    fn malformed_dictionary_extents_and_aliases_reject_before_audio_access() {
        let good = fixture();
        for (at, value) in [
            (12, 0u32),
            (16, 0),
            (20, u32::MAX),
            (40, 1),
            (44, 0),
            (64, 0),
            (64, 20),
            (88, 0),
        ] {
            let mut b = good.clone();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(read(&b).is_err(), "offset {at}");
        }
        let mut b = good.clone();
        b[24] = b'/';
        assert!(read(&b).is_err());
        let mut b = good.clone();
        b[72..75].copy_from_slice(b"one");
        assert!(read(&b).is_err());
        assert!(read(&good[..95]).is_err());
        let mut b = good.clone();
        b.push(0);
        assert!(read(&b).is_err());
    }
}
