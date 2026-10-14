# Continuous engine audio presentation

The first granular mixer is replaced by continuous playback of all six locally
loaded Lacetti PCM recordings through twelve independently advancing voices.
Each category has two engine/exhaust pairs. The reconstructed RPM mapping,
direct1024-entry curves, pair scheduling, crossfade gains and seek transitions
drive the PCM cursor; recordings are no longer repeatedly cut into128ms grains.
Source carsetup verifies lengths, and decoded tuning supplies physical idle/max.
The earlier granular implementation remains a separate diagnostic for comparison.

This is experimental presentation, not 1:1 sound. The original full451770 load,
gear/wobble/rev-limit effects, virtual backend, spatialization, reverb and stereo
routing are not reconstructed. A16ms audio scheduling cadence, linear mono
resampler, direct throttle, initial stopped voices/fade, zero angular-motion
adjustment, neutral Treble, observed Chase bus offsets(-14engine/-7exhaust dB)
and silent input-floor voices are explicit policies. Native observed cooldown
0x3e21cac0 is used; its producer rule remains unknown. Camera3 publication math
has been proved; applying its scalar gains to this mono presentation does not
prove the original Chase2 camera path. No invented EQ is used as a claimed fix.

PCM cursors advance continuously between seek commands at integer source
frequency. Separate accepted seek results seed incoming cursors. Input gain
publication happens before restart, matching the recovered order. Pause/mute
ramp over10ms and then freeze presentation; generation reset reconstructs
presentation state. This freeze policy is independent of original paused
controller behavior. The coherent atomic telemetry mailbox quantizes RPM and
throttle. Audio never writes physics. A0.98 output clamp is a guard, not original
compression; accepted renders hit it zero times.

Existing Rust/Cargo1.99.0/Python3.12.14/NumPy2.3.5 are reused. Fixed-RPM12s plus
1000source ticks/16s render1344000samples per profile. Debug/release WAVs and
telemetry are byte-exact. Source render records875audible blocks/174pair
restarts, no failures/clipping. Paired physical/car/frame/grouped contacts,
world and solver rows remain exact. Pause silence and reset-repeat checks pass.
Dominant40..800Hz peaks across2200/4500/7000rpm are100/157/223Hz; returning
4500rpm decel is156Hz. This demonstrates pitch response, not sound fidelity.
RMS fixed.091254039/source.130709398; peaks.531738281/.609100342.
Maximum adjacent steps.188049316/.165161133 include deliberate abrupt control
changes; they are not a claim of inaudible clicks.

14ordinary tools tests, source mixer units and all-target tools/app Clippy pass.
Private evidence:
- local/audio/continuous-20261014-debug01 and continuous-20261014-release01.
- fixed-rpm.wav SHA256040347390d441c9fb36dd69e57a826dcab45a19f6cbd58b11d08558002d390ae.
- source-drive.wav SHA256dff2efab98c6985b957a0de40b0801651f6dd386ebc173ac5a5f3b62f3c5a66a.
- continuous-20261014-validation-final01.json SHA256f9817cde5afc4aaa56233b04e32df8f88ab6b574b74ee93f1e1b669569f02a49.

The full scheduler service proof is documented in ENGINE_AUDIO_CONTROL.md.
Actual playable/device checks and revision/hash belong in PROTOTYPE_BUILD_UPDATES.md.
Listening against the original is still needed; do not mark F275 fidelity passed.


Owner listening feedback (2026-10-14T18:02:27.273239+00:00): the new sound is "definitely better".
Perceived improvement is now confirmed; a1:1 match remains unverified.
This supports replacing the first granular presentation, not attributing the
old tone to a missing filter or declaring the remaining effects complete.
