# Telemetry-driven engine PCM experiment

14 October 2026, F275. This is a private diagnostic comparison, not an updated
playable mixer or an audible-equivalence pass. Source/code provenance and native
capture details are in [GRID_AUDIO_CAPTURE.md](GRID_AUDIO_CAPTURE.md).

`tools/render_grid_engine_trace.py` reads bounded ET01/ET02 observations and six
already validated local monoPCM16/32kHz Lacetti exports. It never publishes assets
or reads/writes the game process. It produces private48kHz comparison WAVs and a
report with input/output hashes, assumptions and spectral measurements.
The12voice mapping is engine/exhaust ×deceleration/acceleration/loaded ×two banks.
Unknown native resampling/spatial/device/filter behavior is not silently filled
in as verified behavior. Linear PCM interpolation and mono summation are explicit
experimental choices. Source positions outside a recording produce zero.

Three phase experiments remain available. `interpolate` joins observed frame
positions between observations; `advance` advances from every observation;
`continuous` integrates observed integer backend frequency until an inferred
seek. The inferred seek threshold is1024frames and is not claimed to reproduce
the original restart service. Repeated controller observations are collapsed.
The continuous method avoids treating frame-quantized positions as the sample
clock. Its initial phase and seek timing still differ from an observed device
cursor, which has not been captured.

The first ET01 replay used pre-mix input gains and kept the engine sounding
through paused sections. Its spectral mismatch is not a controlled measure of
replacement fidelity. ET02 exposes post-mix0xCC and backend volume/frequency;
these gains suppress paused and quiet loaded voices and preserve unequal engine/
exhaust attenuation. Native bus processing is observed, not yet reconstructed.

Accepted repeatable experiment: `throttle-backend01`, seconds13..17, ET02post-mix
and continuous phase. The backend-unmuted section is14.300074..16.530451s. It
includes acceleration, a shift and possible tyre/gear/wind/ambient sounds.
Original RMS is0.123645616; independently summed voice RMS is0.125574438. Listening
copies use RMS alignment0.984640006, with no additional shared attenuation. This
scale is for comparison only; it is not a fitted native bus rule or a quality
score. Spectra use mean per-channel power (not stereo downmix),8192sample Hann
windows/4096hop, normalized to40..12000Hz power. A previous exploratory report
used downmixed native audio and should not be compared as this metric.

Original125..250Hz fraction is34.52%, replay56.93%; original250..500Hz13.63%,
replay19.59%. Original500..2000Hz is40.17%, replay18.91%. These are remaining
measured differences, not evidence that a specific EQ/filter will solve them.
Other game sounds and native phase/spatial processing are confounders. A quieter
stationary reference and original restart/bus reconstruction are the next steps.
No listening approval or 1:1 claim has been obtained.

Both WAVs and the entire JSON report repeat byte for byte with Python3.12.14 /
NumPy2.3.5. Private artifacts:

- local/audio/native-backend-replay-continuous-20261014-final01/observed-voices.wav SHA256ef611642d04a0b5534e326d8e26de9a8b37dad1c86cc98855ad79d57b19ca26e.
- original-grid.wav SHA256667f0a6c08f8aa1ec7274de0187c3d09ff5b5916b4bc3f075c0a50f6057c8d7b.
- validation.json SHA2565ce2168fa7e4ed085683de01ba122092bbf3b70003ad2f16ab902ec59170b530.
- Repeat directory local/audio/native-backend-replay-continuous-20261014-repeat01.

The backend capture document initially counted51frequency differences when
summing the printed per-voice counts; recounting actual words establishes40of
35,988. The document/log are corrected. This was a reporting arithmetic error,
not a changed capture, dropped mismatch or widened comparison tolerance.

The current playable75fd3ca remains the owner-rejected first granular mixer;
this diagnostic does not replace either launcher or alter driving physics.


Stationary follow-up uses seconds 1..14 of `stationary-idle01`, continuous
backend frequency and observed post gains. Native RMS 0.0536564985 versus
replay 0.0467034709 gives comparison-only alignment 1.148876035. Native
40..125 Hz power fraction is 17.97% versus replay 50.25%; 500..1000 Hz is
28.73% versus 1.25%. Remaining ambient sources, phase and spatial processing
prevent attributing this difference to an engine filter. No fidelity pass.
Private `local/audio/native-stationary-replay-20261014-final01/` contains
observed-voices.wav SHA256 3ed4963ba17e2ddeabcc859ea57f4bc8a44cf84cb4e1df9997a817d913408d41
and original-grid.wav SHA256 24eafdffff54f15e2a763e3d0bf2be7d7c2ab78e76f01ee8a4e03f45cb664986.
