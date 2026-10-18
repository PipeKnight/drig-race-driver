# Desktop gamepad probe

18 October 2026, Claude Code (F415). Probe: `crates/grid-probes/src/bin/grid-pad-probe.rs` (`cargo build --release -p grid-probes --bin grid-pad-probe`). It answers what the game's gamepad backend reports for the owner's pad, and whether the original XInput pad preset's controls exist on it.

- Backend: the game's Bevy gamepad input is bevy_gilrs → gilrs 0.11.2 → gilrs-core 0.6.8 with its default `wgi` feature (Windows.Gaming.Input) [V Cargo.lock / Cargo.toml]. The probe uses the same crate and backend.
- `grid-pad-probe passive [SECONDS]` opens no window: identity, the gilrs code of each preset control, and min/max of every value over the period.
- `grid-pad-probe buttons` is a guided test in a small window. It runs 3 s untouched (values at rest while focused), then asks for each preset control in turn, 4 s each, with Russian prompts. It records which gilrs control moved and its range. The owner runs it with `Тест геймпада.cmd` in the main checkout's `local/pad-probe/` (the probe executable is copied there; sha256 f355077d…ec34).

## Passive result on the owner's PC [V, `local/pad-probe/passive-1.txt`]

- One pad: name and OS name "Xbox 360 Controller for Windows", USB 2dc8:3106 (2dc8 is 8BitDo's vendor id [I: public USB id list]). gilrs UUID is all zeros: WGI's `Gamepad` class gives no GUID, so `mapping_source=Driver` (WGI's own fixed Xbox layout, no SDL mapping). Wired, force feedback supported.
- Every control the XInput pad preset binds (input.rs `xinput_source`) has a gilrs code. A, B, X, Y are buttons 1, 2, 0, 3; LB/RB are 4, 5; Back is 8, Start 9, the stick clicks 10, 11. LT and RT are analogue axes 3 and 4 (gilrs `LeftTrigger2`/`RightTrigger2`, 0..1 under WGI, so they cannot rest at −1 on this backend). The D-pad is buttons, and the sticks are axes 0/1 and 2/5. bevy_gilrs converts each 1:1 to Bevy's `GamepadButton`/`GamepadAxis`, so the preset's mapping table covers this pad. The game's own log agrees: `drive_input_pad ... name="Xbox 360 Controller for Windows" vendor=Some(2dc8) product=Some(3106) preset4=used`.
- Values at rest: **not measured**. In 5 s the probe received no event at all, and gilrs's values stay at their initial 0 (sticks) or absent (buttons) until a first event. From a process without a focused window this cannot separate a perfectly still pad from WGI not delivering readings to unfocused processes [I]. The session could not take the foreground: another session was driving the original game. The guided test's first 3 s (`rest_focused ...` lines) measure it.
- Dead zones in the game: `input::passthrough` sets Bevy's pad dead zone, change threshold and trigger rounding to none, so the original's calibration alone applies (dz 0.2 on steering in preset 4, F321).

## Guided test on the owner's PC (F431) [V, `local/pad-probe/buttons-20261019-021650.txt`]

The owner ran `Тест геймпада.cmd` on 19 October 2026. The pad was wired by USB, the probe was the F415 build (sha256 f355077d…ec34), and its window had the focus.

- **At rest** (first 3 s, untouched): no axis or button event. All four stick axes read 0.0000; every button and both triggers have sent no event, so the game reads them as released. So there is no drift.
- **Caveat: that build filtered.** It built gilrs with its default filters, while bevy_gilrs uses `with_default_filters(false)` [V bevy_gilrs 0.19.1 `lib.rs`]. The default filters add a radial stick dead zone (the driver's, or gilrs's `DEFAULT_DEADZONE` 0.1 when WGI gives none, as here [V gilrs-core 0.6.8 `windows_wgi`: `deadzone: None`]) and a jitter filter. So "0.0000 at rest" means **any stick drift is below 0.1**, not that the raw value the game reads is 0. Where that matters: preset 4 gives steering a 0.2 dead zone (F321), so stick drift below 0.1 cannot steer.
- **Triggers:** LT reached 0.9956 and RT 1.0000. 0.9956 = 255/256, the original's XInput trigger scale `b/256` (maximum 0.996, docs/PLATFORMS_AND_INPUT_RESEARCH.md); RT's 1.0000 presumably comes from WGI's own scaling [I]. Neither trigger rests at −1: WGI reports them as 0..1 `ButtonChanged` values, and the smallest value seen while moving was 0.0414.
- **Sticks:** full −1..1 on all four axes. Small off-axis values while moving one axis (for example LeftStickY −0.06 while LeftStickX went to −1) are normal cross-talk.
- **Buttons and D-pad:** every preset control produced its event with the codes from the passive run. Most presses landed one prompt late (the owner said so), so the per-step "want/seen" pairs are shifted by one step. The button lines show only the release (0.0) inside the step that saw it.
- **Probe fixed since:** `grid-pad-probe` now builds gilrs as the game does (`with_default_filters(false)`, report header `pad_probe version=2 filters=none`). The copy in `local/pad-probe/` is replaced (sha256 92b95384…fd29). A future run of `Тест геймпада.cmd` measures the unfiltered rest values.
- Not measured: the pad over USB-C OTG on the phone (the owner may try it; P11b, docs/ANDROID.md "Gamepads").
