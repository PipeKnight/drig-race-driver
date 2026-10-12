# Try Lacetti at Donington

The experimental keyboard driving build is available now. A complete manually driven lap has not been verified. Road support and vehicle movement work; body/barrier collisions, race rules and lap timing are unfinished. Leaving the supported road can make the car fall, and unsupported simulation conditions can pause it. **R** returns to the starting position.

## Launch on this PC

1. Open the Reassembly project folder.
2. Double-click **Play GRID Prototype.cmd** in its root. The release executable is already built on this PC; no additional tools are needed to try it.
3. Wait for the track to load, then click the game window once to give it keyboard focus.
4. Start gently down the straight. Release throttle and brake before corners; tap steering to make small corrections. This build uses keyboard controls and automatic transmission. Its source initialization is based on assisted settings, but original-game handling fidelity has not been calibrated against the recording.
5. Try following the circuit. There is no lap counter yet; reaching the start line again is a manual playtest result, not an automatically validated lap.

| Key | Action |
| --- | --- |
| Up / W | Throttle |
| Down / S | Brake |
| Left / A, Right / D | Steer |
| Space | Handbrake |
| P | Pause / resume |
| R | Reset car and simulation to the start |
| Escape | Close |

The window title shows speed, tick number and pause/failure status. If movement pauses after a simulation error, note the title or keep the diagnostic log, then press R. Each launch saves output under ignored `local/playtest/`; game-derived evidence stays private. The original Steam installation is read for resources.

Useful first-drive feedback: whether left/right and braking feel correct, whether the camera lets you see the next corner, and the first place where you cannot continue. If you complete a lap, say so and keep the log; it will guide a reproducible lap validation. Recording video is optional for this first try.

## Other checkouts

Build with `cargo build --release -p grid-app`, then launch with `target\release\grid-app.exe --drive --game-root "YOUR_GRID_FOLDER"`. The launcher accepts a game-folder argument and defaults to `C:\Program Files (x86)\Steam\steamapps\common\Grid`. It reads locally purchased resources; no game assets or compiled toolchain are bundled in the repository.

Implementation and earlier verification: [source driving prototype](SOURCE_DRIVING.md). New contact-query/manifold research does not become driving behavior until separately integrated and checked.
