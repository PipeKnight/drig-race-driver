# DRIG roadmap

What comes next after release v0.1.0, in the order the owner approved (F867, 2026-11-19, and later owner requests up to 2026-11-24). Each step links its rows in the [fidelity backlog](FIDELITY_BACKLOG.md), which holds the details, the evidence and what is already done. The order can change; the backlog and [PROJECT_LOG.md](../PROJECT_LOG.md) hold the current state.

## Rule for every build

DRIG builds never contain game files or DLC files. Everything, including the DLC and any cut content, is read from the player's own legitimate copies on their own disk. Releases ship only DRIG's own code, its licence notices and the third-party libraries and font it includes.

## Order

1. **Frame rate.** At least 100 fps in every one-second window of a race, with the 1 % low at least 80, at resolutions up to 2560x1440 (F862, F883). Where it stands, cell by cell: [PERFORMANCE.md, "Status matrix against the 100 fps goal (F862)"](PERFORMANCE.md#status-matrix-against-the-100-fps-goal-f862). No speed-up may add visible artefacts (F645) or change car parts, damage or detachment (owner rule of 2026-11-20, F887).
2. **The original UI.** Restore the original menus and front-end screens. The new look and display options come with it: bloom and white balance (row 140) and HDR output (row 141).
3. **Rain, the DLC and cut content**, each as an option that leaves the original mode unchanged:
   - rain (row 136);
   - the official DLC, read from the player's own copy (row 137);
   - cut and unused content as an extras mode (row 138).
4. **Audio.** Spatial audio as in the original, with reverb and surround, and an optional HRTF mode for headphones (row 142).
5. **Haptics.** Realistic haptics and adaptive triggers for pads that have them, such as the DualSense (row 146, low priority).
6. **Distribution.** A notice inside the game when a newer release is out, then in-app updates (row 131), and the rest of rows 129-135: the portable data folder, the player build without developer files, one-file releases, player options and the native Linux build. Several of these shipped in v0.1.0; the rows say which parts are open.
7. **Career window settings.** `--career` should follow the window, fullscreen and resolution options like the rest of the game (row 147).

Fidelity work (handling, AI, damage, graphics and the rest of the backlog) continues alongside these steps.
