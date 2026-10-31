# Race music: codec, stem manager and port sizing (research)

31 October 2026, Claude Code (car-series; the owner asked for the music research part first, sized,
with no port yet). Static analysis of `local/compatibility/grid-unpacked.exe` (cache `local/re-cache`)
and of the purchased `audio/Music/*.WIM` and `audio/CMSamples.dic`. A subagent surveyed the code. I
re-checked the codec table and the channel layout myself. GRID was not launched, and no decoded audio
was kept.

Labels: **[O]** read in code or data; **[I]** interpretation.

This corrects docs/AUDIO_RESEARCH.md §28, which had the WIM data as an unknown compressed codec and
`raceend.WIM` as the race-end music.

## 1. The WIM format: 8-channel μ-law

- **[O]** 0x43EC80 (`cmr6_audio_manager.cpp`) sets up stream bank 0:
  - width 1 byte, interleave 8;
  - rate field 0x107D00: 32000 plus bit 0x100000, which is the dic "looped" flag 0x10 [I: looped];
  - format 0x2001, buffer 0x180000.
- **[O]** The platform check 0xA2D330 allows interleaved streams only as mono PCM (0x1101) or
  μ-law (0x2001). For 0x2001 it sets the "compression factor" to 2.0. The validator 0xA22480
  takes up to 8 channels and sizes the buffer from rate·interleave·2/factor.
- **[O]** The decoder 0xA27A20 (UpdateInterleave) is sample-interleaved with an 8-byte stride and
  no block or channel headers. Each byte goes through the 256-entry int16 table at 0xE50938.
  - That table is G.711 μ-law. I re-checked all 256 entries [O].
  - The exe has no IMA or MS ADPCM step table.
  - Each channel feeds its own mono OpenAL source.
- **[O data]** The files hold whole frames:
  - 28,311,552 bytes = 110.6 s (music01, 02, 06, 07, 08, replay1);
  - 31,457,280 bytes = 122.9 s (music03, 04, 05, raceend, replay2).
- **[O data]** music01 decodes smoothly, from 30 s in. Lag-1 autocorrelation:
  - channels 0–6: 0.97–0.98 (RMS 3000–7500);
  - channel 7: −0.19 (RMS 1640), a high-frequency layer [I: percussion].
- **[I]** The extension letter names the codec: WIP is PCM, WIM is μ-law, WIO is Ogg and WIA is
  ADPCM.

## 2. Stems [O data, I roles]

- **Race tracks** (music01, music03; correlations 30–40 s): channel 0 correlates with 4, and 1
  with 5, at 0.5–0.8. Channels 2, 3, 6 and 7 are independent. So they are stems, not stereo pairs.
- **replay1:** four independent stereo pairs, (0,1) (2,3) (4,5) (6,7). That is four songs, one
  per pair.
- **raceend:** one stereo song repeated on all 8 channels, with silence at the start and the end.

## 3. The music manager (`cmr6_audio_music_manager`) [O unless marked]

- **Enabled flag +0x121.** It is not the green light. It means "race music on":
  - the option handler 0x412980 sets it and picks the track music0%i, i = rand & 7 + 1;
  - the end of a replay (0x420D20) sets it again.
- **Start (0x4414E0).** Music starts once the player's speed, averaged over 5 samples taken
  every 0.033 s, reaches 5 mph. In Le Mans and in a replay it starts at once.
- **Opening (0x42A9C0).** 15 ticks after the chooser runs, it creates a master voice and 7
  substream voices (index << 8). They start at −96 dB.
- **Group [I].** The voices use the group pointer at +0x5D4, which is Race Music (10). Front-end
  music uses +0x5F4, Music (18). Both carry the Music Volume trim (docs/PAUSE_MENU.md).
- **Stop (0x420BD0).** It stops all 8 voices. It has no fade of its own [I]; the group-mode
  levels duck it (Paused −48, …).
- **Speaker positions (race mode).** Channels 0 and 1 are left, 4 and 5 right, 2 and 3 centre;
  6 is at x −0.25 and 7 at +0.25. In replay and Le Mans mode, even channels are left and odd
  channels right.
- **Per-stem levels, every tick (dB, clamped to [−48, 0]):**
  - stems (0,4) and (1,5) crossfade by 0.025 per tick. The direction comes from a race-progress
    comparison at +0x60 [I: lap against total laps];
  - channel 2: the base layer, always on;
  - channel 3: the speed layer, silent below 80 mph and full at 150 mph, capped at −2 dB;
  - channel 6: −12 dB per place behind first, muted below 3rd;
  - channel 7: only while the player leads;
  - all stems: a fade-in with speed up to 50 mph, and a crash duck of 48 − 15·timer dB scaled by
    0.25, 0.0625 or 0.5 per stem, held until the player regains speed (+0x5C/+0x68, terms not
    fully named).
- **Replay.** With race track n it plays replay((n − 1)/4 + 1). Only the pair (n & 3)·2 is at
  0 dB; the rest are at −48. So the replay plays one of the eight songs that replay1 and replay2
  hold.
- **raceend.WIM** is started only by `startLemansMusic` (0x420D90), called from 0x434440 on a time
  condition. It plays pair 0/1. It is Le Mans music, not the race-end music.
- **The option (`race_music_option`, 0x4A9E80, `DataRaceMusicToggle`).**
  - The default is registered as 1 (`push 1` at 0x4AC7AF), only when the profile has no value.
    The getter clamps it to 2.
  - Value 0 → 0x420DF0: all music off.
  - Value 1 → 0x420E40: race music only while +0x13C is set. Its only setter, 0x412930, has no
    callers.
  - Value 2 → 0x412980: a random race track.
  - [I, to be checked against the options screen's labels] So with the default 1 the race
    plays no music.
- **Front end.** `femusic`, `femusic1` and `femusic2` (`.WIO`) are Ogg Vorbis sample banks
  (32 kHz, dic flags 0x30), loaded through the "femusic" bank by 0x4412A0. `femusic3` is not in
  CMSamples.dic.
- **Garage.** `gar_level*.WIA` are garage ambience and one-shots at 40 kHz, not music [I: IMA or
  Xbox ADPCM in 72-byte stereo blocks].

## 4. What a port needs, and its size

| Part | Work | Estimate |
|---|---|---|
| Decoder | 8-channel μ-law from the WIM file, streamed and looped | 0.5 d |
| Stream voices | 8 mono voices on Race Music 10 at the speaker positions above, with the Music Volume trim and the mix modes | 0.5 d |
| Stem rules | crossfade, speed, place and lead layers, fade-in, crash duck; needs the +0x60/+0x5C/+0x68 inputs named first (static) | 1–1.5 d |
| Triggers | the option (and its default: check the UI labels), the 5 mph start, Le Mans `raceend`, the replay pair | 1 d |
| Front-end Ogg | `femusic*.WIO` with an Ogg Vorbis crate (a new dependency to pin) | 0.5 d |
| Garage ADPCM | optional, not music | 1 d |

Total for the race music: about 3–3.5 days; with the front end, about 4. Before porting:
- name the three open inputs (+0x60, +0x5C, +0x68);
- settle the option's default, because if the default is "off", race music is opt-in.

The new addresses still need rows in `docs/re/symbols.tsv`, which `mine_symbols.py` takes from this
doc.
