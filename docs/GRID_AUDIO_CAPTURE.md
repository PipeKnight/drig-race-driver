# Original GRID audio capture

Status on 14 October 2026: the selected-process recorder works. Only a menu
pilot has been captured; this is **not yet a Lacetti engine reference**.

`tools/capture_grid_audio.cpp` uses Microsoft's process-loopback API to include
the identified private GRID process and its children. It opens no microphone,
has no whole-system fallback, injects nothing into GRID and changes no audio
settings. PID, process creation time and the established private executable path
must match. Duration is bounded to 0.5–60 seconds. New WAV/packet files must be
under ignored `local/`; existing files are rejected.

The existing MSVC 14.44.35207 and Windows SDK 10.0.26100 build the recorder with
`tools/build_grid_audio_capture.cmd`; no downloaded recorder or SDK was needed.
Output is stereo PCM16 at 48 kHz after Windows' format conversion. Packet rows
retain flags and QPC timestamps. This is a reference of the game's audible
output through this Windows backend, not a copy of the source PCM stream.

The two-second menu pilot contains 95,040 frames (1.98 seconds), 198 packets and
nonzero audio. No discontinuity or timestamp-error flags occurred; successive
QPC timestamps agree exactly with the packet durations. Device position was
always zero and is treated as unavailable. The first validator incorrectly
required advancing device positions; it now validates QPC timing instead.

Private evidence:

- `local/audio/native-reference-20261014-01/menu-pilot-validation.json`, SHA256
  `21c04c73f452e1e1e26fbb568ccd9a4894200d176d9bbe8c6d20cb40fbbd6ea2`.
- `local/audio/capture-guards-20261014-final01/validation.json`, SHA256
  `ec7e2012d3a6f1cb0d8df88977cd8942538374e09202fe0204739bbbee983168`:
  eight incorrect-process/time/duration/output attempts reject before recording;
  the existing pilot and packet file remain byte for byte unchanged.
- `local/audio/native-reference-20261014-01/session.json` identifies private
  GRID PID 44472, started 2026-10-14T10:05:43.1319198Z. Nineteen user files were
  backed up before launch. These identifiers expire when the process restarts.

Computer Use reached Race Day using the established guarded AutoHotkey 2.0.29
bindings. The direct launcher redirected to Steam; launching the already
installed private copy through its observed Explorer entry worked. Navigation
later reported intervening input, a minimized window and unsuccessful window
restoration. No further input was retried; offline reconstruction continued.
The latest observed selection was Freestyle Drift, before reaching Touring Cars.

## Correcting the loaded audio module record

The initial `audio-modules.json` used module-reported `System32` filenames from
a 64-bit shell. GRID is 32-bit: these names are redirected and those hashes did
**not** identify the actual loaded Windows/OpenAL backing files.

`tools/read_grid_audio_modules.ps1` now queries `K32GetMappedFileNameW` at each
loaded module's base address, resolves its device path and verifies PE machine
014C before hashing the real file. The corrected record is
`local/audio/native-reference-20261014-01/audio-mapped-modules.json`, SHA256
`e83d3447d29ed3e76f8e87e8e856aacde519c171ab794db463249456b386cea6`.

| Module | Actual location | Version | Bytes |
|---|---|---|---:|
| DSOUND.dll | Windows/SysWOW64 | 10.0.26100.9492 | 458752 |
| OpenAL32.dll | Windows/SysWOW64 | 6.14.0357.24 | 109080 |
| wrap_oal.dll | Windows/SysWOW64 | 2.2.0.5 | 444952 |
| binkw32.dll | Private GRID copy | 1.8r | 166912 |

Loaded modules alone do not establish the active audio backend. Its selected
interface, camera processing and any filtering still require evidence.

Next capture a controlled Lacetti/Donington idle, acceleration and coast with
camera, RPM/load and voice state recorded. Opponents, music and resampling must
be accounted for in comparisons. No audible-fidelity conclusion comes from the
menu pilot or the recorder's timing validation.

Primary API references:
[process-loopback parameters](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ns-audioclientactivationparams-audioclient_process_loopback_params),
[activation parameters](https://learn.microsoft.com/en-us/windows/win32/api/audioclientactivationparams/ns-audioclientactivationparams-audioclient_activation_params),
[ActivateAudioInterfaceAsync](https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-activateaudiointerfaceasync),
[Microsoft application-loopback sample](https://github.com/microsoft/Windows-classic-samples/tree/main/Samples/ApplicationLoopback).


## F275: synchronized native engine observations, 14 October 2026

The previous reference process had exited. Computer Use 26.1007.71326 reopened
only the private compatibility copy through its observed Explorer entry after
Sky launch failed (`accessibility window-opened handler did not become ready`).
The first resumed input then reported a physical Escape stop; that turn ended.
The owner explicitly requested continuation, and fresh observation/control worked.
The original Steam installation was not launched or edited by these actions.
Current settings were backed up in `local/audio/native-reference-20261014-02/user-before`.

The new session records PID38636, start16:27:26.5056468UTC and the same private
executable SHA256 b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6.
AutoHotkey2.0.29 reused the existing guarded macro source; server25220 is bound
to that process. All actual game input went through Computer Use F-key dispatch.
The selected race is Touring Cars/Chevrolet Lacetti/Nevada Autosport,
Donington Park/Grand Prix Circuit/3 laps/Normal/Random, twelve cars, Chase view.
No audio settings were changed. Assists were previously observed all On with
Automatic transmission; this session also visibly shifted automatically.

`tools/read_grid_engine_controller.ps1` checks session path/start/hash/base and
three live code prefixes against the private analysis executable. It scans only
committed private read/write regions, with a maximum256MiB and64 candidates.
The signature is the constructor's paired root/profile ownership relation;
additional component/vehicle/length checks reject unrelated objects. Reads and
outputs remain bounded, and every output is newly created under ignored local/.
It never writes, suspends or injects into a game process. A menu128MiB scan found
no candidate within its limit; that is not an exhaustive absence result.
A race256MiB scan found twelve validated controllers, one player. Its
`ScanComplete=false` explicitly retains the search-limit qualification.

The player's controller36ed9f20 has the purchased Lacetti477036/319292 recording
lengths, twelve32kHz voice slots and the expected engine/exhaust pairs. Its
Chase camera mode is observed as2, not the camera3 scope of the earlier whole-gain
proof. Both emitter ranges are10..175 and both resource trims are negative zero.
Do not relabel camera3 as Chase or use the earlier fixture ranges1..50 as native.
Sequential capture is not a suspended/atomic snapshot. Pausing physics does not
freeze all audio-controller bytes; a repeat read can legitimately differ.

The twenty-second `throttle-pilot01` recording ended16:37:43.705391UTC, before
the macro began16:37:45UTC. Although recording and macro each succeeded, this
pilot did not capture that acceleration. It is retained as a timing failure,
not accepted as matched evidence. The subsequent optional recorder mode reads
engine state in the same process/audio event loop using the shared QPC clock.
A shadowed-member warning was fixed before the /W4/WX build passed.

`capture-grid-audio ... local/NEW.wav PLAYER_CONTROLLER_DECIMAL` additionally
creates `NEW.wav.engine.bin`. Before audio activation it checks the player's
root/profile/component links and Lacetti lengths; every sample rechecks ownership.
It reads only the0x340controller, three engine scalars, camera mode, root Treble,
two physical-motion vectors and twelve0x80voice properties. No game code runs
in the recorder. Existing audio-only invocation remains supported.

ET01 uses a32-byte little-endian header: magic, record size, count, controller
address, u64QPC frequency, PID, reserved zero. Each2440-byte record contains
u64begin/end QPC,0x340controller, engine speed/maximum/idle f32, camera mode u32,
root Treble f32, eight motion f32, twelve0x80voice objects and u32controller-repeat
comparison. Reads are sequential. A matching repeated controller does not prove
that every other object belongs to the same simulation frame.

The accepted `throttle-synced01` recording spans29.99s/1,439,520stereoPCM16 frames
at48kHz. All2,999packets have valid, gap-free timestamps and no discontinuities.
The guarded throttle runs16:40:51..53UTC within this recording; its normal macro
completion and final Continue-selected pause menu were observed. There are2,999
engine observations over29.9804435s, intervals8.2022..11.7253ms and reads20.9..273.6us.
2,990controller repeats match exactly; nine differ and remain explicitly admitted
as sequential observations. Engine speed spans519.673828..874.271240 against
maximum890.117920 (native angular-speed units, not a direct RPM label).

All captured camera modes are2 and root Treble stays zero. All four loaded
voice gains remain exactly-48; normal/deceleration voices vary up to-0.334190dB.
This is live support for a mixing difference from the rejected first mixer.
It does not establish filter response, backend cutoff, exact PCM phase or audible
1:1 equivalence. The process recording includes other GRID voices, tyres,
ambience and possible music; no isolated-player-engine claim is made.
The current playable still uses the earlier experimental granular mixer.

Private evidence (all under local/audio/native-reference-20261014-02):

- race-controller-search01.json SHA256 c35dc06b833a7b09c7f692f769230d5a717a4ab6c60b84ecb5a67c81b64461d2.
- player-paused02.json SHA256 f80f61763a05f680ffe8a34da941e85252effb168f69940cacdd6dc9282c2044.
- throttle-synced01.wav.engine.bin SHA256033448eab8e4c053b573e0bcadb947f075a6bca9057e52d511a9b0720d644c88.
- throttle-synced01-engine-validation.json SHA2565b0529155420c90a5e8175fb345bce0d9132fc99efa5abb4030e1366b5dfeb9c.
- throttle-synced01-audio-validation.json SHA256f1cb56b94eb9c823179f85a759b666cde4ee3c9f19f2a0100405bda24140e429.

MSVC14.44.35207/SDK10.0.26100/DevPrompt17.14.41 rebuilt the recorder with no new
installation; executable SHA256c0ca94e74cb55a85a5e0ae7d8c288508c4ddb3c573db07745b7f7cccf0e20f2c.
Python3.12.14/NumPy2.3.5 independently validate packet/trace structure and bounds.
Twelve process/start/duration/output/controller guard cases reject before
recording and preserve the existing WAV/packet files byte for byte. Audit
`local/audio/capture-engine-guards-20261014-final01/validation.json` SHA256
ee18fdcf0415e22c05e409596a2efde65696ff02badbdd96af445d7741472def.

Next compare telemetry-driven playback of all six purchased PCM recordings with
this native output, distinguish phase/resampling/mixing/spatial differences, and
finish original restart scheduling before replacing the playable mixer. Keep
all WAVs and raw object observations private.


### ET02: post-mix and backend observations

The ET01 voice0x14 volume is an input to the mixer, not its backend output.
Aligned A233D0 passes voice0xCC/0xD0 to A25D90; volume0x14 is not passed directly.
A paused read shows all twelve0xCC values at-96.300003dB even while the controller's
input volumes continue to vary. Therefore the first input-gain-only PCM replay
kept engine sound across paused sections and is not a valid matched mix.
This does not invalidate the input-gain/controller math proof; it establishes
an additional downstream stage that the playable has not reconstructed.

The extended reader verifies six code prefixes and captures0xE0voice records,
then validates each voice0x68 backend slot against the bounded table count.
It reads only each selected slot's0x300..0x36Ftail, not the whole voice table.
The table base/stride derive from unchanged A25D90/A2C080:0xE9C7F0/0x370.
Additional fixed reads cover64backend-global bytes, nine group attenuation/filter
words and16configuration bytes. Zero backend IDs are not silently substituted;
only the native0xA000001 stopped sentinel admits an absent tail.

ET02 retains the32-byte header and first900record bytes, expands twelve voice
records to0xE0, then stores twelve0x70backend tails, u32controller-repeat flag,
64backend globals,36group-Treble bytes,16filter configuration bytes and36group
filter bytes. Record size is5088. The inspector retains ET01 compatibility.
The same source recorder rebuilt with the existing pinned MSVC toolchain passes
/W4/WX; SHA25669526668909caad3fe9f54597b5e09ad271639895a94cfc69d3ff109c0225b80.

The30s `throttle-backend01` capture includes normal guarded throttle16:51:37..40UTC,
visible automatic third gear and final macro pause.1,439,520audio frames/2,999packets
again have no discontinuity/timestamp gaps.2,999engine observations span29.9800296s,
intervals8.1566..11.8354ms, sequential reads33.6..318.1us and2,981exact controller
repeats. The unmuted backend interval is14.300074..16.530451seconds relative to
first audio packet (224observations). All35,988voice0xCC words equal their backend
volume words exactly. Backend frequencies agree with rounded rate*pitch except
40of35,988sequential observations; those are retained, not asserted atomic.

All four loaded backend voices remain-96.300003dB in this sample. During the
active interval the greatest engine-bay post-minus-input offset is about-14dB,
exhaust about-7dB. Transition/quiet states have larger attenuation. This is an
observed route-specific offset, not yet a ported universal bus rule. All backend
masks are0x2003, root/backend Treble0, global filter mask6. The selected group
attenuation word is0x80000001 (negative subnormal near zero). The traced filter
formula would round to gainHF1 from these captured values. That inference is not
an actual OpenAL parameter query and does not establish audible filter response.
These observations give no support for inventing a strong Chase-view lowpass EQ.

Private evidence under local/audio/native-reference-20261014-02:

- player-backend-paused01.json SHA25627f828f06226755b321237fdc9fab5888f93bec4b90be124f6a040898fd18854.
- throttle-backend01.wav.engine.bin SHA2561781343d499c6f9415d4d3c2647fdfb193d4507d2f18e5a14d67cefa82cf9cd0.
- throttle-backend01-engine-validation.json SHA256b823ce2ef2d363009c79a897a74c07a5030afa0e93b4283b080e2994e471998a.
- throttle-backend01-audio-validation.json SHA25652ab7bddb3d145c5009d041aca6f32b46b541cb79020ab39df60c3611378932b.
- backend-parameter-summary.json SHA25618077dc2d9e95911532a5a14e6b018ea95f4b5e7e60b0983f65017af76eb8b5b.

Twelve recorder guards pass again and preserve WAV, packet and engine files.
`local/audio/capture-backend-guards-20261014-final01/validation.json` SHA256
3dc4f9b262efec3d5e7187baa0815234dda99fa72c35b10d14d71f980b514d8b.
No new tool installation or playable replacement accompanies this diagnostic.
