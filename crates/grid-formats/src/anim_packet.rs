//! Bounded ordinary L4/S4 animation packet payloads with preserved float32 keys.
//!
//! The caller supplies a bounded PSSG leaf and explicit parent metadata. This module
//! does not change PSSG admission or decode KA/B1. Sampling accepts explicit caller
//! bank coordinates and returns raw values; it does not construct clocks or poses.
//! Native layout offsets are checked metadata, not serialized-payload offsets.
use std::str;

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHANNELS: usize = 512;
const MAX_KEYS_PER_CHANNEL: usize = 100_000;
const MAX_TOTAL_KEYS: usize = 1_000_000;
const MAX_NAME_BYTES: usize = 256;

/// Supported ordinary packet family; other codecs are rejected explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    L4,
    S4,
}
impl Kind {
    fn semantic(self) -> &'static str {
        match self {
            Self::L4 => "translation",
            Self::S4 => "rotation_quaternion",
        }
    }
}
/// Metadata from the enclosing packet and its bounded raw leaf.
#[derive(Clone, Copy, Debug)]
pub struct Envelope<'a> {
    /// Exact parent node type, paired with `leaf_type`.
    pub parent_type: &'a str,
    /// Exact ordinary data-leaf type; no unknown-family fallback is provided.
    pub leaf_type: &'a str,
    /// Declared native allocation alignment; the supported layout requires 16.
    pub parent_alignment: u32,
    /// Declared native allocation size, distinct from serialized payload length.
    pub native_size: usize,
}
/// Payload cursor position and a bounded-reader rejection reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    /// Cursor position relative to the supplied payload, not the outer file.
    pub offset: usize,
    pub reason: &'static str,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at payload 0x{:x}: {}", self.offset, self.reason)
    }
}
impl std::error::Error for Error {}

/// Result of decoding one supported ordinary payload.
pub type Result<T> = std::result::Result<T, Error>;
/// One finite saved timestamp and four values, preserved without normalization.
#[derive(Clone, Copy, Debug)]
pub struct Key4 {
    /// Authored bank-time scalar; no wall-clock unit or frame grid is inferred.
    pub time: f32,
    pub value: [f32; 4],
}
/// Source channel order, target name, native spans and serialized key sequence.
#[derive(Debug)]
pub struct Channel4<'a> {
    pub target: &'a str,
    /// Native time-array offset, never an index into the serialized payload.
    pub time_offset: u32,
    /// Native float4-array offset, never an index into the serialized payload.
    pub value_offset: u32,
    pub keys: Vec<Key4>,
}
/// One ordinary payload; each channel semantic is fixed by `kind`.
#[derive(Debug)]
pub struct Packet<'a> {
    pub kind: Kind,
    pub native_size: usize,
    pub channels: Vec<Channel4<'a>>,
}

/// Refusal from the strict sampling domain, distinct from serialized-reader errors.
/// Duplicate times remain valid resources; this API requires increasing sample times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleError {
    MissingChannel,
    EmptyChannel,
    TooManyKeys,
    NonFiniteCoordinate,
    NonFiniteKey { index: usize },
    NotStrictlyIncreasing { right: usize },
    NonFiniteSpan { right: usize },
    NonFiniteNumerator,
    NonFiniteFraction,
    NonFiniteDot,
    AcosDomain,
    SphericalArithmetic,
    NonFiniteOutput,
}
impl std::fmt::Display for SampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SampleError {}

impl Packet<'_> {
    /// Sample one ordinary channel at an explicit caller-supplied bank coordinate.
    /// Returns raw four-lane values: no clock conversion, looping, sign flip,
    /// normalization, pose/reference transform, output-slot map or rig binding.
    ///
    /// The full channel must be finite, within the reader's key-count bound and
    /// strictly increasing. Finite out-of-range coordinates clamp the interval
    /// fraction; a one-key channel preserves its words directly. Duplicate-time
    /// resources remain readable but are refused here. These validation errors
    /// are authored safety policy, not original malformed-input behavior.
    ///
    /// S4 uses raw-dot spherical weights below its source threshold, otherwise
    /// unnormalized LERP. Host f64 acos/sin with f32 staging is not an original
    /// x87/CRT bit-parity guarantee. No multi-key endpoint copy is substituted.
    pub fn sample(
        &self,
        channel: usize,
        coordinate: f32,
    ) -> std::result::Result<[f32; 4], SampleError> {
        let keys = &self
            .channels
            .get(channel)
            .ok_or(SampleError::MissingChannel)?
            .keys;
        let interval = sample_interval(keys, coordinate)?;
        let Some((left, fraction)) = interval else {
            return Ok(keys[0].value);
        };
        let a = keys[left].value;
        let b = keys[left + 1].value;
        let weights = match self.kind {
            Kind::L4 => [1.0 - fraction, fraction],
            Kind::S4 => s4_weights(a, b, fraction)?,
        };
        let [w0, w1] = weights;
        let value = match self.kind {
            Kind::L4 => std::array::from_fn(|lane| {
                let left = a[lane] * w0;
                let right = b[lane] * w1;
                left + right
            }),
            Kind::S4 => [
                a[0] * w0 + b[0] * w1,
                b[1] * w1 + w0 * a[1],
                b[2] * w1 + w0 * a[2],
                a[3] * w0 + w1 * b[3],
            ],
        };
        if value.iter().any(|v| !v.is_finite()) {
            return Err(SampleError::NonFiniteOutput);
        }
        Ok(value)
    }
}

// B2DA90 search/previous exact-key interval; strict guards reuse the validated
// diagnostic domain, not the resource reader's duplicate-time admission policy.
fn sample_interval(
    keys: &[Key4],
    coordinate: f32,
) -> std::result::Result<Option<(usize, f32)>, SampleError> {
    if keys.is_empty() {
        return Err(SampleError::EmptyChannel);
    }
    if keys.len() > MAX_KEYS_PER_CHANNEL {
        return Err(SampleError::TooManyKeys);
    }
    if !coordinate.is_finite() {
        return Err(SampleError::NonFiniteCoordinate);
    }
    for (index, key) in keys.iter().enumerate() {
        if !key.time.is_finite() || key.value.iter().any(|v| !v.is_finite()) {
            return Err(SampleError::NonFiniteKey { index });
        }
        if index > 0 {
            if key.time <= keys[index - 1].time {
                return Err(SampleError::NotStrictlyIncreasing { right: index });
            }
            if !(key.time - keys[index - 1].time).is_finite() {
                return Err(SampleError::NonFiniteSpan { right: index });
            }
        }
    }
    if keys.len() == 1 {
        return Ok(None);
    }
    let (mut left, mut right) = (0, keys.len() - 1);
    while right - left > 1 {
        let mid = left + (right - left) / 2;
        if coordinate <= keys[mid].time {
            right = mid;
        } else {
            left = mid;
        }
    }
    let numerator = coordinate - keys[left].time;
    if !numerator.is_finite() {
        return Err(SampleError::NonFiniteNumerator);
    }
    let fraction = numerator / (keys[right].time - keys[left].time);
    if !fraction.is_finite() {
        return Err(SampleError::NonFiniteFraction);
    }
    Ok(Some((left, fraction.clamp(0.0, 1.0))))
}

fn s4_weights(
    a: [f32; 4],
    b: [f32; 4],
    fraction: f32,
) -> std::result::Result<[f32; 2], SampleError> {
    // B32B80 source product/sum association: z+y+w+x; no abs(dot)/sign flip.
    let zy = a[2] * b[2] + a[1] * b[1];
    let zyw = zy + a[3] * b[3];
    let dot = zyw + a[0] * b[0];
    if !dot.is_finite() {
        return Err(SampleError::NonFiniteDot);
    }
    let mut weights = [1.0 - fraction, fraction];
    if dot < f32::from_bits(0x3f7fbe77) {
        // Original has no protective clamp. Explicit refusal is diagnostic policy.
        if dot < -1.0 {
            return Err(SampleError::AcosDomain);
        }
        let theta = f64::from(dot).acos() as f32;
        let sine = f64::from(theta).sin() as f32;
        let inverse = 1.0 / sine;
        let angles = [theta * weights[0], theta * weights[1]];
        weights = angles.map(|angle| (f64::from(angle).sin() as f32) * inverse);
        if [
            theta, sine, inverse, angles[0], angles[1], weights[0], weights[1],
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(SampleError::SphericalArithmetic);
        }
    }
    Ok(weights)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn error(&self, reason: &'static str) -> Error {
        Error {
            offset: self.at,
            reason,
        }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(len)
            .ok_or_else(|| self.error("range overflow"))?;
        let result = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| self.error("truncated payload"))?;
        self.at = end;
        Ok(result)
    }
    fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn text(&mut self) -> Result<&'a str> {
        let len = self.u32()? as usize;
        if !(1..=MAX_NAME_BYTES).contains(&len) {
            return Err(self.error("name length"));
        }
        let text = self.take(len)?;
        if !text.is_ascii() {
            return Err(self.error("name ASCII"));
        }
        str::from_utf8(text).map_err(|_| self.error("name ASCII"))
    }
}

/// Decode exactly one bounded ordinary payload with checked native layout.
///
/// Limits are 16 MiB per payload/native allocation, 512 channels, 256 bytes per
/// name, 100,000 keys per channel and 1,000,000 keys in total. Timestamps must be
/// finite, nonnegative and nondecreasing. Equal times remain resource data; this
/// reader does not establish their playback/interpolation policy.
pub fn read<'a>(payload: &'a [u8], envelope: Envelope<'_>) -> Result<Packet<'a>> {
    let mut r = Reader {
        bytes: payload,
        at: 0,
    };
    let kind = match (envelope.parent_type, envelope.leaf_type) {
        ("NeAnimPacket_L4", "NeAnimPacketData_L4") => Kind::L4,
        ("NeAnimPacket_S4", "NeAnimPacketData_S4") => Kind::S4,
        _ => return Err(r.error("unsupported or mismatched ordinary packet family")),
    };
    if payload.len() > MAX_BYTES || envelope.native_size > MAX_BYTES || envelope.native_size == 0 {
        return Err(r.error("packet size limit"));
    }
    if envelope.parent_alignment != 16 || !envelope.native_size.is_multiple_of(16) {
        return Err(r.error("parent native alignment"));
    }
    let count = r.u32()? as usize;
    let alignment = r.u32()?;
    if !(1..=MAX_CHANNELS).contains(&count) || alignment != 16 {
        return Err(r.error("channel count/alignment"));
    }
    let header_end = count
        .checked_mul(20)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| r.error("native header overflow"))?;
    if header_end > envelope.native_size {
        return Err(r.error("native header escape"));
    }
    let mut channels = Vec::new();
    channels
        .try_reserve_exact(count)
        .map_err(|_| r.error("channel allocation"))?;
    let mut spans = Vec::new();
    spans
        .try_reserve_exact(count * 2)
        .map_err(|_| r.error("span allocation"))?;
    let mut total_keys = 0usize;
    let mut max_end = 0usize;
    for _ in 0..count {
        let target = r.text()?;
        let semantic = r.text()?;
        if semantic != kind.semantic() {
            return Err(r.error("family semantic mismatch"));
        }
        let key_count = r.u32()? as usize;
        if !(1..=MAX_KEYS_PER_CHANNEL).contains(&key_count) {
            return Err(r.error("key count limit"));
        }
        total_keys = total_keys
            .checked_add(key_count)
            .ok_or_else(|| r.error("total keys overflow"))?;
        if total_keys > MAX_TOTAL_KEYS {
            return Err(r.error("total keys limit"));
        }
        let time_offset = r.u32()?;
        let value_offset = r.u32()?;
        let t = time_offset as usize;
        let v = value_offset as usize;
        if t < header_end || v < header_end || !t.is_multiple_of(4) || !v.is_multiple_of(16) {
            return Err(r.error("native span alignment/header overlap"));
        }
        for (start, stride) in [(t, 4usize), (v, 16usize)] {
            let end = key_count
                .checked_mul(stride)
                .and_then(|n| start.checked_add(n))
                .ok_or_else(|| r.error("native span overflow"))?;
            if end > envelope.native_size {
                return Err(r.error("native span escape"));
            }
            spans.push((start, end));
            max_end = max_end.max(end);
        }
        // Check serialized bytes before reserving key storage. Native offsets are metadata,
        // never indexes into the serialized payload's interleaved time/value records.
        let byte_count = key_count
            .checked_mul(20)
            .ok_or_else(|| r.error("serialized keys overflow"))?;
        let raw = r.take(byte_count)?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(key_count)
            .map_err(|_| r.error("key allocation"))?;
        let mut previous = None;
        for row in raw.as_chunks::<20>().0 {
            let mut f = [0.0f32; 5];
            for (value, bytes) in f.iter_mut().zip(row.as_chunks::<4>().0) {
                *value =
                    f32::from_bits(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
            }
            if f.iter().any(|v| !v.is_finite()) {
                return Err(r.error("nonfinite key"));
            }
            if f[0] < 0.0 || previous.is_some_and(|old| old > f[0]) {
                return Err(r.error("nonmonotonic time"));
            }
            // Equal timestamps remain accepted as in the checked Python reader.
            // Evaluator denominator policy is outside this resource-only contract.
            previous = Some(f[0]);
            keys.push(Key4 {
                time: f[0],
                value: [f[1], f[2], f[3], f[4]],
            });
        }
        channels.push(Channel4 {
            target,
            time_offset,
            value_offset,
            keys,
        });
    }
    if r.at != payload.len() {
        return Err(r.error("trailing serialized bytes"));
    }
    spans.sort_unstable();
    if spans.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(r.error("overlapping native spans"));
    }
    let rounded_end = max_end
        .checked_add(15)
        .ok_or_else(|| r.error("native tail overflow"))?
        & !15;
    if rounded_end != envelope.native_size {
        return Err(r.error("native tail size"));
    }
    Ok(Packet {
        kind,
        native_size: envelope.native_size,
        channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn u(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_be_bytes());
    }
    fn text(out: &mut Vec<u8>, value: &str) {
        u(out, value.len() as u32);
        out.extend_from_slice(value.as_bytes());
    }
    fn env(kind: Kind, native_size: usize) -> Envelope<'static> {
        let (parent_type, leaf_type) = match kind {
            Kind::L4 => ("NeAnimPacket_L4", "NeAnimPacketData_L4"),
            Kind::S4 => ("NeAnimPacket_S4", "NeAnimPacketData_S4"),
        };
        Envelope {
            parent_type,
            leaf_type,
            parent_alignment: 16,
            native_size,
        }
    }
    // Returns count/time-offset/value-offset field start and first serialized key.
    fn fixture(kind: Kind, count: u32) -> (Vec<u8>, usize, usize, usize) {
        let mut out = Vec::new();
        u(&mut out, 1);
        u(&mut out, 16);
        text(&mut out, "joint");
        text(&mut out, kind.semantic());
        let fields = out.len();
        u(&mut out, count);
        u(&mut out, 36);
        u(&mut out, 48);
        let keys = out.len();
        for index in 0..count {
            for v in [index as f32, 1.0, -0.0, 3.0, 0.0] {
                u(&mut out, v.to_bits());
            }
        }
        (out, fields, keys, if count == 1 { 64 } else { 80 })
    }
    fn set(data: &mut [u8], at: usize, value: u32) {
        data[at..at + 4].copy_from_slice(&value.to_be_bytes());
    }
    fn encode(packet: &Packet<'_>) -> Vec<u8> {
        let mut out = Vec::new();
        u(&mut out, packet.channels.len() as u32);
        u(&mut out, 16);
        for c in &packet.channels {
            text(&mut out, c.target);
            text(&mut out, packet.kind.semantic());
            u(&mut out, c.keys.len() as u32);
            u(&mut out, c.time_offset);
            u(&mut out, c.value_offset);
            for key in &c.keys {
                u(&mut out, key.time.to_bits());
                for value in key.value {
                    u(&mut out, value.to_bits());
                }
            }
        }
        out
    }
    #[test]
    fn ordinary_roundtrip_and_negative_zero_bits() {
        for kind in [Kind::L4, Kind::S4] {
            for count in [1, 2] {
                let (raw, _, _, native) = fixture(kind, count);
                let packet = read(&raw, env(kind, native)).unwrap();
                assert_eq!(encode(&packet), raw);
                assert_eq!(packet.channels[0].keys[0].value[1].to_bits(), 0x80000000);
            }
        }
    }
    #[test]
    fn every_truncated_prefix_and_trailing_byte_rejects() {
        for kind in [Kind::L4, Kind::S4] {
            let (mut raw, _, _, native) = fixture(kind, 2);
            for end in 0..raw.len() {
                assert!(
                    read(&raw[..end], env(kind, native)).is_err(),
                    "accepted prefix {end}"
                );
            }
            raw.push(0);
            assert_eq!(
                read(&raw, env(kind, native)).unwrap_err().reason,
                "trailing serialized bytes"
            );
        }
    }
    #[test]
    fn unsupported_families_and_mismatched_envelopes_reject() {
        let (raw, _, _, native) = fixture(Kind::L4, 2);
        for name in [
            "NeAnimPacketData_L4_KA",
            "NeAnimPacketData_S4_KA",
            "NeAnimPacketData_B1",
            "NeAnimPacketData_L1",
            "unknown",
        ] {
            let mut e = env(Kind::L4, native);
            e.leaf_type = name;
            assert!(read(&raw, e).is_err());
        }
        let mut e = env(Kind::L4, native);
        e.parent_type = "NeAnimPacket_S4";
        assert!(read(&raw, e).is_err());
        let mut e = env(Kind::L4, native);
        e.parent_alignment = 8;
        assert!(read(&raw, e).is_err());
        assert!(read(&raw, env(Kind::S4, native)).is_err());
    }
    #[test]
    fn malformed_counts_names_alignment_and_ranges_reject() {
        let (raw, fields, _, native) = fixture(Kind::L4, 2);
        for (at, value) in [
            (0, 0),
            (0, 513),
            (4, 8),
            (8, 0),
            (8, 257),
            (fields, 0),
            (fields, 100001),
            (fields + 4, 0),
            (fields + 4, 37),
            (fields + 8, 16),
            (fields + 8, 49),
            (fields + 8, 80),
            (fields + 8, 0xfffffff0),
        ] {
            let mut bad = raw.clone();
            set(&mut bad, at, value);
            assert!(
                read(&bad, env(Kind::L4, native)).is_err(),
                "mutation {at}/{value}"
            );
        }
        let mut bad = raw.clone();
        bad[12] = 255;
        assert!(read(&bad, env(Kind::L4, native)).is_err());
        let mut bad = raw;
        bad[21] = b'X';
        assert!(read(&bad, env(Kind::L4, native)).is_err());
    }
    #[test]
    fn finite_nondecreasing_time_and_values_are_required() {
        for kind in [Kind::L4, Kind::S4] {
            let (raw, _, keys, native) = fixture(kind, 2);
            for component in 0..5 {
                for bits in [0x7f800000, 0xff800000, 0x7fc00001] {
                    let mut bad = raw.clone();
                    set(&mut bad, keys + component * 4, bits);
                    assert!(read(&bad, env(kind, native)).is_err());
                }
            }
            let mut bad = raw.clone();
            set(&mut bad, keys, (-1.0f32).to_bits());
            assert!(read(&bad, env(kind, native)).is_err());
            let mut bad = raw.clone();
            set(&mut bad, keys, (2.0f32).to_bits());
            assert!(read(&bad, env(kind, native)).is_err());
            let mut equal = raw;
            set(&mut equal, keys + 20, 0);
            assert!(read(&equal, env(kind, native)).is_ok());
        }
    }
    #[test]
    fn native_tail_and_header_are_exact() {
        let (raw, _, _, native) = fixture(Kind::L4, 2);
        for size in [0, 16, 64, 79, 96, MAX_BYTES + 16] {
            assert!(read(&raw, env(Kind::L4, size)).is_err());
        }
        assert!(read(&raw, env(Kind::L4, native)).is_ok());
        let mut big = vec![0; MAX_BYTES + 1];
        big[..raw.len()].copy_from_slice(&raw);
        assert!(read(&big, env(Kind::L4, native)).is_err());
    }
    #[test]
    fn distinct_channels_native_overlap_rejects() {
        let mut raw = Vec::new();
        u(&mut raw, 2);
        u(&mut raw, 16);
        let mut second_time_field = 0;
        for (index, name) in ["first", "second"].iter().enumerate() {
            text(&mut raw, name);
            text(&mut raw, "translation");
            u(&mut raw, 1);
            if index == 1 {
                second_time_field = raw.len();
            }
            u(&mut raw, 56 + index as u32 * 4);
            u(&mut raw, 64 + index as u32 * 16);
            for v in [0.0f32, 1.0, 2.0, 3.0, 0.0] {
                u(&mut raw, v.to_bits());
            }
        }
        assert!(read(&raw, env(Kind::L4, 96)).is_ok());
        set(&mut raw, second_time_field, 56);
        assert_eq!(
            read(&raw, env(Kind::L4, 96)).unwrap_err().reason,
            "overlapping native spans"
        );
    }
}

#[cfg(test)]
mod sampling_tests {
    use super::*;
    fn packet(kind: Kind, keys: Vec<Key4>) -> Packet<'static> {
        Packet {
            kind,
            native_size: 0,
            channels: vec![Channel4 {
                target: "Authored",
                time_offset: 0,
                value_offset: 0,
                keys,
            }],
        }
    }
    fn pair(kind: Kind, a: [f32; 4], b: [f32; 4]) -> Packet<'static> {
        packet(
            kind,
            vec![
                Key4 {
                    time: 0.0,
                    value: a,
                },
                Key4 {
                    time: 1.0,
                    value: b,
                },
            ],
        )
    }
    fn near(a: f32, b: f64) {
        assert!((f64::from(a) - b).abs() < 2e-6, "{a} != {b}");
    }
    #[test]
    fn one_key_l4_s4_preserves_raw_words_and_input() {
        let value = [-0.0, f32::from_bits(1), -2.0, 3.0];
        for kind in [Kind::L4, Kind::S4] {
            let p = packet(kind, vec![Key4 { time: 7.0, value }]);
            for coordinate in [-100.0, 7.0, 100.0] {
                assert_eq!(
                    p.sample(0, coordinate).unwrap().map(f32::to_bits),
                    value.map(f32::to_bits)
                );
            }
            assert_eq!(
                p.channels[0].keys[0].value.map(f32::to_bits),
                value.map(f32::to_bits)
            );
        }
    }
    #[test]
    fn l4_quarter_uses_all_four_unnormalized_lanes() {
        let p = pair(Kind::L4, [1.0, 2.0, -3.0, 5.0], [5.0, 6.0, 1.0, 13.0]);
        assert_eq!(p.sample(0, 0.25).unwrap(), [2.0, 3.0, -2.0, 7.0]);
    }
    #[test]
    fn s4_near_dot_lerp_is_not_normalized() {
        let p = pair(
            Kind::S4,
            [0.0, 0.0, 0.0, 1.0],
            [0.02, 0.0, 0.0, (1.0f32 - 0.02f32 * 0.02f32).sqrt()],
        );
        let value = p.sample(0, 0.5).unwrap();
        let norm = value.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>();
        assert!((norm - 1.0).abs() > 5e-5);
        near(value[0], 0.01);
    }
    #[test]
    fn s4_negative_pair_is_not_shortest_path() {
        let a = [0.0, 0.0, 0.0, 1.0];
        let b = [0.6, 0.0, 0.0, -0.8];
        let long = pair(Kind::S4, a, b).sample(0, 0.25).unwrap();
        let theta = (-0.8f64).acos();
        near(long[0], (theta * 0.25).sin());
        near(long[3], (theta * 0.25).cos());
        let flipped = pair(Kind::S4, a, b.map(|v| -v)).sample(0, 0.25).unwrap();
        assert!((long[0] - flipped[0]).abs() > 0.5);
    }
    #[test]
    fn s4_threshold_is_strict_and_dot_over_one_stays_linear() {
        let a = [0.0, 0.0, 0.0, 1.0];
        let threshold = f32::from_bits(0x3f7fbe77);
        assert_eq!(
            s4_weights(a, [0.04, 0.0, 0.0, threshold], 0.25).unwrap(),
            [0.75, 0.25]
        );
        assert_ne!(
            s4_weights(
                a,
                [0.04, 0.0, 0.0, f32::from_bits(threshold.to_bits() - 1)],
                0.25
            )
            .unwrap()[0]
                .to_bits(),
            0.75f32.to_bits()
        );
        near(
            pair(Kind::S4, a, [0.0, 0.0, 0.0, 1.1])
                .sample(0, 0.5)
                .unwrap()[3],
            1.05,
        );
    }
    #[test]
    fn s4_source_dot_order_changes_the_sample() {
        let a = [1e8, -1e8, 1.0, 1.0];
        let b = [1.0; 4];
        let value = pair(Kind::S4, a, b).sample(0, 0.25).unwrap();
        // Generic XYZW accumulation gives dot2/LERP; source z+y+w+x gives dot0.
        assert!((value[0] - (a[0] * 0.75 + 0.25)).abs() > 1e7);
    }
    #[test]
    fn endpoints_and_exact_interior_use_the_source_interval() {
        let keys = vec![
            Key4 {
                time: 0.0,
                value: [0.0; 4],
            },
            Key4 {
                time: 1.0,
                value: [1.0; 4],
            },
            Key4 {
                time: 3.0,
                value: [2.0; 4],
            },
        ];
        assert_eq!(sample_interval(&keys, -20.0), Ok(Some((0, 0.0))));
        assert_eq!(sample_interval(&keys, 1.0), Ok(Some((0, 1.0))));
        assert_eq!(sample_interval(&keys, 20.0), Ok(Some((1, 1.0))));
        let p = packet(Kind::L4, keys);
        assert_eq!(p.sample(0, -20.0).unwrap(), [0.0; 4]);
        assert_eq!(p.sample(0, 20.0).unwrap(), [2.0; 4]);
    }
    #[test]
    fn sampling_refusals_do_not_change_reader_admission() {
        let p = pair(Kind::S4, [0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, -1.1]);
        assert_eq!(p.sample(0, 0.5), Err(SampleError::AcosDomain));
        assert_eq!(p.sample(1, 0.5), Err(SampleError::MissingChannel));
        assert_eq!(p.sample(0, f32::NAN), Err(SampleError::NonFiniteCoordinate));
        // Real serialized duplicate-time fixtures remain admitted by the reader.
        // Sampling is a separate stricter operation; do not change resource policy.
        for kind in [Kind::L4, Kind::S4] {
            let mut raw = Vec::new();
            for word in [1u32, 16] {
                raw.extend_from_slice(&word.to_be_bytes());
            }
            for text in ["Authored", kind.semantic()] {
                raw.extend_from_slice(&(text.len() as u32).to_be_bytes());
                raw.extend_from_slice(text.as_bytes());
            }
            for word in [2u32, 36, 48] {
                raw.extend_from_slice(&word.to_be_bytes());
            }
            for values in [[0.0f32, 1.0, -0.0, 3.0, 4.0], [0.0, 5.0, 6.0, 7.0, 8.0]] {
                for value in values {
                    raw.extend_from_slice(&value.to_bits().to_be_bytes());
                }
            }
            let before = raw.clone();
            let (parent_type, leaf_type) = match kind {
                Kind::L4 => ("NeAnimPacket_L4", "NeAnimPacketData_L4"),
                Kind::S4 => ("NeAnimPacket_S4", "NeAnimPacketData_S4"),
            };
            let decoded = read(
                &raw,
                Envelope {
                    parent_type,
                    leaf_type,
                    parent_alignment: 16,
                    native_size: 80,
                },
            )
            .unwrap();
            assert_eq!(decoded.channels[0].keys.len(), 2);
            assert_eq!(decoded.channels[0].keys[0].time.to_bits(), 0);
            assert_eq!(decoded.channels[0].keys[1].time.to_bits(), 0);
            assert_eq!(
                decoded.channels[0].keys[0].value[1].to_bits(),
                (-0.0f32).to_bits()
            );
            assert_eq!(
                decoded.sample(0, 0.0),
                Err(SampleError::NotStrictlyIncreasing { right: 1 })
            );
            assert_eq!(raw, before);
        }
    }
}
