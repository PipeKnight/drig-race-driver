# Lacetti engine audio

## F273: dictionary and PCM validation, 14 October 2026

The installed `audio/CMSamples.dic` is DIC1/version 2.00, 72,576 bytes,
SHA256 `f0b9e2974227aecfeaef83f7b2ad31c5e37dc10e20a9d7ff29680f6b9488f45a`.
Its independently inspected layout is a 16-byte header, 124 24-byte bank
descriptors, contiguous 24-byte sample descriptors and an 8-byte trailer per
bank. Bank descriptors contain table offset/count/name; sample descriptors
contain offset, low 16-bit sample rate, opaque 16-bit flags and name. Trailers
contain final bank extent and extension. The header word at offset 4 remains
opaque; no checksum interpretation is established.

All 124 banks / 2,858 samples consume the dictionary exactly: 105 WIP, eight
WIO and eleven WIM banks. All declared file extents match the installation.
An initial root-only search missed the eleven music files; they are under
`audio/Music`, so this was a lookup error, not a missing installation resource.

`audio/eng_lac.WIP` is 5,093,480 bytes, SHA256
`be043a1fcfbdbbf33bfee7135ef20ea91954d95e5df9a5f0bcc23db4e8a0d842`.
Its six zero-flag, 32,000 Hz sample extents cover the entire bank:

| Sample | First byte | End (exclusive) | Mono frames |
|---|---:|---:|---:|
| dn_en | 0 | 638588 | 319294 |
| dn_ex | 638588 | 1277176 | 319294 |
| up_en | 1277176 | 2231252 | 477038 |
| up_en_load | 2231252 | 3185328 | 477038 |
| up_ex | 3185328 | 4139404 | 477038 |
| up_ex_load | 4139404 | 5093480 | 477038 |

The bounded Rust reader accepts this selected WIP/zero-flag PCM16 mono domain.
It rejects unknown decoding flags, other extensions, odd PCM extents, aliased
tables, duplicate/unsafe names, invalid rates and unconsumed bytes. Unknown flags
in the general dictionary stay opaque. This does **not** establish decoding for
mechanical, multichannel or music banks. The CLI validates every selected sample
before creating a new private directory and never modifies game resources.

Independent Python reconstruction agrees with all 2,983 Rust TSV rows. Debug
and release WAVs are byte-identical; Python wave inspection and installed PyAV
decode all 2,546,740 PCM words exactly to the source bytes. No resampling, gain
or trimming is involved. The owner listened to `dn_en.wav` and reported
"Sounds like an engine slowing down". This is useful subjective confirmation,
separate from proof of original native playback or mixing behavior.

Private evidence:

- `local/audio/rust-{debug,release}-eng-lac-20261014-final-01/`
- `local/audio/lacetti-audio-validation-20261014-final-01.json`, SHA256
  `61a61a982dfbd8b92f7950a9f14603efc5dbc4bb25aea42df11dd9ebc0ec2e09`
- `local/audio/audio-cli-guards-20261014-01/validation.json`: both profiles reject
  absent bank, mechanics flags and an existing output without changing output.
- `local/audio/audio-reader-libs-20261014.txt`: 208 ordinary library tests pass.
- `local/audio/audio-reader-clippy-20261014.txt`: workspace/all-target checks pass.

Tools reused: Rust/Cargo 1.99.0; Python 3.12.14; PyAV 19.0.0 from ignored
`local/tools/video-python`; FFmpeg libraries avutil 61.1.102, avcodec/avformat
63.1.102, avdevice 63.1.102, avfilter 12.1.102, swscale 10.1.102,
swresample 7.1.102. No ffmpeg/ffprobe/vgmstream/sox executable was on PATH;
none was installed because the existing decoder suffices. NumPy is installed;
Matplotlib is absent and was unnecessary.

## Provenance and unresolved native behavior

Primary references consulted, with no external extractor code copied, installed
or executed: [vgmstream format registry](https://github.com/vgmstream/vgmstream/blob/master/src/formats.c)
reserves WIP for TXTH;
[EGO format request](https://github.com/vgmstream/vgmstream/issues/1369) reports
historical extractor channel/rate issues; and
[EEnot dictionary source](https://github.com/NC22/EEnot-Audio-extractor/blob/master/source/dicutils.pas)
suggests PCM16 but has heuristics which we did not adopt. Our boundaries and
selected PCM interpretation were checked independently against local files.

`audio/cars/lacengine.ini` SHA256
`cfa8887466c35ad5e9a2287c8e670a300952567f1c49e288ec57a1a63a7a04f8`
contains two 1,024-element RPM/time table pairs and repeated Loop sections.
Idle Length 8270381 and Throttle Length 12356305 have unverified units; they
must not be called sample counts or microseconds. Original table indexing,
loop selection, load/gear transitions and full mixer fidelity remain open.

Fresh Ghidra 12.1.4 / JDK 21.0.9 surveys identify original INI loader 423BB0
and startup caller 4376B0. Tables are written at owner offsets 1190/2190
(idle) and 4BEC/5BEC (throttle). 415D10 initializes a 256-entry trig table.
Startup resource fields 1178/117C come from resource object +30/+34;
their meanings are unproved. A tentative RPM interpretation was discarded.
Displacement search found only loader references; indirect consumers may use
inner pointers, so this does not establish absence of consumers.

Private pseudocode and aligned instructions are in `local/behavior/20261014T0033026477292Z/`,
`20261014T0034514347436Z/`, `20261014T0037030062673Z/`,
`20261014T0038275021538Z/` and `local/audio/audio-profile-loader-aligned-20261014.*`.
Initial searches wrongly expected NUL after TimeFromRevs; the actual string
ends with `=`. Corrected references are recorded in
`local/audio/engine-string-references-20261014-corrected.json`. No original
audio loader/mixer execution or native-equivalence claim has been made.

Next: an explicitly independent, bounded RPM/throttle mixer using these local
decoded layers, read-only vehicle telemetry, pause/reset/mute controls, offline
waveform checks and a refreshed playable build. Physics must remain unaffected.
