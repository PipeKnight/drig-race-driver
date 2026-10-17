# Try Lacetti at Donington

The experimental keyboard driving build is available now. On 12 October the owner reported completing a full Donington lap with Lacetti. That first lap is a manual playtest result; automated lap validation is unfinished. Road support and vehicle movement work; body/barrier collisions, race rules and lap timing are unfinished. Leaving the supported road can make the car fall, and unsupported simulation conditions can pause it. **R** returns to the starting position.

## Launch on this PC

1. Open the Reassembly project folder.
2. Double-click **Play GRID Prototype.cmd** in its root. The release executable is already built on this PC; no additional tools are needed to try it.
3. Wait for the track to load, then click the game window once to give it keyboard focus.
4. Start gently down the straight. Release throttle and brake before corners; tap steering to make small corrections. This build uses keyboard controls and automatic transmission. Its source initialization is based on assisted settings, but original-game handling fidelity has not been calibrated against the recording.
5. Try following the circuit. There is no lap counter yet; reaching the start line again is a manual playtest result, not an automatically validated lap.

| Key | Action |
| --- | --- |
| Up / W | Throttle |
| Down / S | Brake; hold near rest to select reverse and drive backward |
| Left / A, Right / D | Steer |
| Space | Handbrake |
| Escape or P (pad: Start) | Pause menu, as in the original: Continue, Instant Replay (not yet), Restart, Options (sound, screen, wheel controls), Retire from Race / Finish Test Drive (closes the app). Arrows/Enter/Escape and Ctrl (Reset Vehicle); pad D-pad or left stick, A, B and X ([pause menu](PAUSE_MENU.md)) |
| R | Restart the event from the start |
| X / Z | Gear up / down (manual gearbox only) |

**Driver aids** follow the original options ([research](DRIVING_ASSISTS_RESEARCH.md)): Traction Control, Stability Control, Braking Assist and Automatic/Manual transmission, plus keyboard or wheel steering. The default is every aid on, automatic, keyboard. Put them in an `[assists]` section of `localsettings.ini`, for example `tcs=off stability=off braking=off gearbox=manual` ([settings file](SETTINGS.md)); the older `localssists.txt` and `set GRID_PLAY_ASSISTS=...` still work. The app takes `--assists SPEC` or `--assists-file PATH` directly. In manual, reverse is Z from neutral; braking does not select reverse.

The window title shows speed, gear, tick number and pause/failure status. **HB** means the simulation received the Space handbrake command; gear **R** means reverse. While reversing, Up/W brakes and selects forward again near rest. Hold Space while releasing throttle to check the rear-wheel handbrake. Its controlled runtime and Bevy keyboard tests pass; the owner subsequently confirmed HB appears and the car slows.

If movement pauses after a simulation error, note the title or keep the diagnostic log, then press R. Each launch saves output under ignored `local/playtest/`; game-derived evidence stays private. New logs record control changes and periodic pose/velocity/gear/wheel-force samples, plus pause/reset/exit events. The first build's two logs contained startup diagnostics only, so they cannot reconstruct that lap. The original Steam installation is read for resources.

Useful first-drive feedback: whether left/right and braking feel correct, whether the camera lets you see the next corner, and the first place where you cannot continue. If you complete a lap, say so and keep the log; it will guide a reproducible lap validation. Recording video is optional for this first try.

## Other checkouts

An optional **Play GRID Body Contacts.cmd** mode now joins the source body mesh
to Donington's packed-track and separate CQTC boundary contacts. A 600-tick
scripted barrier-sliding check passes in both Rust builds and the rendered app;
the corresponding packed-only control falls off the supported world. Selected
collision damage now runs with recovered source wheel regions and owned RNG;
unsupported notification events can still pause driving. See the
[damage integration and current limits](SOURCE_WHEEL_DAMAGE_STARTUP.md). The private
sampling file is already prepared on the owner's PC; no installation is needed.

Build with `cargo build --release -p grid-app`, then launch with `target\release\grid-app.exe --settings settings\practice.ini` (or `race.ini`, `time-trial.ini`, `body-contacts.ini`). The game folder is found through the Steam libraries; `[game] root` in `local\settings.ini`, a launcher argument or `--game-root` overrides it ([settings file](SETTINGS.md)). It reads locally purchased resources; no game assets or compiled toolchain are bundled in the repository.

Implementation and earlier verification: [source driving prototype](SOURCE_DRIVING.md). New contact-query/manifold research does not become driving behavior until separately integrated and checked.

First-session findings and control follow-up: [12 October playtest](PLAYTEST_2026_10_12.md).
