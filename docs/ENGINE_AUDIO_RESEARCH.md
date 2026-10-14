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

## F274: independent runtime mixer (offline verified; playable checks pending)

The first replacement mixer uses exterior dn_ex/up_ex/up_ex_load PCM. It is
an independently chosen presentation policy, not a port of the original mixer.
The source 1,024-entry tables are interpolated over [0,1] using N-1 indexing to
select bounded source windows. Unknown native Length units are not used.
Two 128 ms Hann windows overlap by 50%; source-rate/pitch correction uses the
paired RevsFromTime table with bounded ratios. Acceleration and load layers
crossfade with the requested throttle. The normalized RPM input is original
engine angular speed at vehicle1894 divided by maximum speed17A4. No audio
result feeds simulation; the initial source engine reports zero speed until
started, and zero speed is silent.

Output is 48 kHz mono, with about 25 ms RPM/load smoothing, a 10 ms fade for
pause/mute/failure, conservative .28 gain and bounded soft limiting. This does
not establish calibrated loudness or native engine pitch fidelity. Reset
restarts grain ownership with a fade in; a short reset transition may remain
perceptible. Terminal wheel damage removes the requested throttle load while
retaining whatever engine speed the simulation actually reports.

The Bevy audio decoder reads a single atomic 64-bit telemetry word every64
samples: normalized RPM16bits, throttle8bits, audible1bit, reset generation32
bits. Invalid telemetry silences. Audio failure during loading warns and leaves
driving usable. P pauses, R resets, M toggles engine sound; mute persists across
car resets. Offscreen renders disable AudioPlugin and never open a sound device.
An explicit --audio-check runs a bounded one-second real-device sink check
without opening a game window. Queuing a player is distinct from observing an
active sink; neither proves the owner's subjective sound quality.

Existing Bevy0.19.1 audio dependencies were absent (offline fetch rejected
missing bevy_audio). Authorized Cargo fetch added27 locked packages; selected
runtime versions are bevy_audio0.19.1, rodio0.22.2, cpal0.17.3,
dasp_sample0.11.0. Cargo.lock pins package checksums; no original installation
or sound settings changed. Enabling Windows audio required recompiling shared
Bevy/Windows dependencies. The first --audio-check patch missed a main.rs
context and was reapplied using the actual argument-validation boundary.

The draft diagnostic completed1,000 ordinary source-car steps alongside a
second runtime with identical input. Complete car/physical/frame, grouped body/
contact/world words and solver rows agree at every tick. It also constructs a
12-second fixed-RPM/pause/reset waveform and16-second actual-RPM waveform.
This is not a new native-game capture or full original mixer comparison.
Final source identities, repeat/profile checks, real-device output and playable
refresh are still pending; do not describe this section as a released build.

Final offline mixer evidence: `local/audio/mixer-{debug,release}-20261014-final01/`.
Both12-second fixed-RPM waves match byte-for-byte (SHA256
`766fa898a95c9f4d569f74d52c257f910c4e8e8929403ff6c916ebe42c839753`),
as do both16-second actual-vehicle waves (SHA256
`d548d319b280097dd2b8eb5e12faabc6b866d9d226212d1ec59080445fb48e87`).
All1,000 telemetry rows also match. Stopped-engine frames are zero, pause is
zero after20ms, reset reproduces the initial96,000 frames exactly. Fixed-speed
FFT dominant peaks62/141/219Hz rise with requested2200/4500/7000RPM; this is
only a monotonic signal check, not proof of calibrated pitch. Peaks remain
influenced by the granular window/repetition policy; pulse/loop artifacts and
source-to-RPM calibration remain listening/research questions. Fixed-wave
peak.174134 and actual-drive peak.167481 are bounded; subjective loudness is
not established. The owner answered "I am not sure" to the mixed audition;
quality remains unconfirmed, unlike the earlier decoded slowing-engine sample.

All211 ordinary library tests pass. Constant-chunk Clippy feedback was corrected
using as_chunks; the eighth Bevy presentation system parameter has an explicit
resource-ownership lint allowance. All-target Clippy passes. The first Bevy
sound test wrongly expected sound immediately after reset/unmute, although the
source engine had zero angular speed. The fixture now accelerates before
expecting sound; this did not require changing driving physics. Private failed
log `engine-audio-bevy-controls-20261014-final01.txt` is retained. Initial
release dependency/test warmup completed in11m28s, separately from publication.

Aligned native loader evidence also corrects a draft reading of the voice
stride: it is0x540, not0x6c. Five idle voices at3190 precede descriptor4BD0;
five throttle voices at6BEC precede descriptor862C. This static observation
does not establish original runtime consumption or Length units.

Current offline audit `local/audio/mixer-validation-20261014-final02.json`
SHA256 `a446be97e522ee29a0c4623a74751811fbe0511902c4242029f778bdaf97c574`
pins final authored source/dependency/diagnostic identities. It supersedes the
first audit, whose app-test identity preceded the stopped-engine fixture fix.
`engine-audio-bevy-controls-20261014-final02.txt` passes the actual Bevy input/
mailbox/decoder pause, mute, reset, zero-speed silence, failure and restart
sequence with purchased sources and no opened sound device. Final library log
`engine-audio-libs-20261014-final01.txt`, all-target Clippy log
`engine-audio-clippy-20261014-final02.txt`, and formatting pass. Playable helper
will separately check all source-driving and Bevy regressions before release.
