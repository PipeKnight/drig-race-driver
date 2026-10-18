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

## Next

The owner runs `Тест геймпада.cmd` (about 90 s) and sends the `buttons-*.txt` it writes.
