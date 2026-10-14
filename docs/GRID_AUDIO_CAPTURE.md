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
