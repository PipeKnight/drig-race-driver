# AI choreographer and catch-up: original against the port (F439)

19 October 2026, Claude Code. Static analysis only: Ghidra 12.1.4 headless `-readOnly` on the
private analysis copy and Capstone 5.0.6 scans of the research copy; shipped data read with the
port's readers. No game launch.

This document compares how the original paces the AI field relative to the player with what the
port does today. The detailed research is in
[AI_MISTAKES_PACING_RESEARCH.md](AI_MISTAKES_PACING_RESEARCH.md) (§2.6, §4) and in the owner's
untracked research copy `docs/AI_CHOREOGRAPHER_CATCHUP_RESEARCH.md` (main checkout, not edited
here); the port is `crates/grid-tools/src/ai_pacing.rs`
([AI_DRIVER_PORT.md](AI_DRIVER_PORT.md), "Mistakes, failures and race pacing").

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data; **[L]** live
observation; **[I]** inference.

## 1. Which catch-up config a race uses (new) [V/D/L]

The race manager selects one `catchup.xml` config by `user` name (0x5D5530). The name comes
from the game mode and the event descriptor [V, decompile `cu1/5D5530`; format strings at
0xCE205C/0xCE2054/0xCE2048/0xCE2040]:

| Game mode | Name |
|---|---|
| 0 (GRID World) | `gw_%d_star`, + `_h2h` for head-to-head |
| 5 (Race Day) | `rd_%d_star` |
| 1 | `network` (or `none`) |
| 6 | `first_race` |
| other | `none` |

`%d` is **event desc+0x68 + 1**, and `_touge` is appended when the competition format
(desc+0x4C) is 5 or 6 (pro_touge, midnight_touge; DISCIPLINES.md §1.1).

**desc+0x68 is the star level.**

- The same descriptor block desc+0x68..+0x88 is the event's `difficulty` row. Flashback
  setup (caller 0x58AC70 → 0x6AF7C0) copies it and reads desc+0x84 as `available_flashbacks`
  and desc+0x88 as `flashback_bonus` [V].
- The names run `rd_1_star`…`rd_6_star` [D], one per star level 0..5 (Basic…Ninja), so the
  field is the 0-based star level, not the row id (1..24) or the rank (1..4) [V structure].
- Race Day Normal is star level 1. The live Race Options screen (Difficulty NORMAL) showed
  "FLASHBACKS AVAILABLE: 4", the level-1 count [L, 18 October screenshots], and Normal's
  `ai_ratio` 0.606 is the level-1 row (FLASHBACK.md §2.1).

So **a Normal Race Day race uses `rd_2_star`**, and a Normal Pro Touge `rd_2_star_touge`. The
port had assumed `rd_3_star` (AUTHORED). The two configs differ in one value [D]:

| Config | min/max distance | power at min/max | reverse |
|---|---|---|---|
| `rd_1_star` | −30 / 100 m | 1.0 / 0.4 | 50 / 400 m → 1.0 / 1.2 |
| `rd_2_star` | 50 / 200 m | 1.0 / **0.45** | same |
| `rd_3_star` | 50 / 200 m | 1.0 / 0.5 | same |

All three: minSpeed 12 m/s, minRaceDistance 500 m, useLeadForMaxDistance on. An AI car more than
200 m ahead of the player now runs at 0.45 of its engine torque instead of 0.5.

**Port (F439):** `ai_pacing::race_day_catch_up(difficulty, touge)` builds the name from the star
level of the AI difficulty (`flashback::star_level`). `catchup = on` (and the default) and the
new `catchup = touge` resolve to it after the whole `[ai]` section is read. An explicit name
still selects that config. The settings files now say `catchup = on`; the two Haruna touge files
say `catchup = touge`, and so do the event menu's Midnight Touge and Pro Touge events. Catch-up only acts with a human reference car, so headless AI-only runs
are unchanged.

## 2. Original systems and port status

| System (original) | What it does | Port |
|---|---|---|
| Catch-up (catchup.xml, 0x90A000–0x917000, race manager 0x5C5000) | Engine torque factor per AI from its gap to the player, clamped 0.1–2.0, written to engine +0xE8 | Ported (`catch_up_power`, `torque_factor`); config name 1:1 since F439 |
| Choreographer events (ai_choreographer.xml, 0x401000–0x40C000) | Mistakes (brake_lock, brake_spin, powerout_spin), tyre blow-outs, engine failures on AI cars ahead of the player | Ported (triggers, windows, chances from the `Mistakes` characteristic) |
| Conveyor belt (0x407940, 0x403E20) | One front-third car slowed (A = 0.7), one back-third car sped up (A = 1.1) | Ported; AUTHORED: single-class fields only, 0-based places |
| Race-start factors (0x401AA0) | Engine × 1.05 ahead of the player in the first 100 m; 1.25 grip for the first 750 m | Ported (`grip_factor`) |
| Off-line grip assist (0x404C90) | Grip up to +25 % inside / +50 % outside when off the racing line in a bend | Ported (`grip_factor`) |
| Corner-speed A/B (driver+0x3154/+0x3158, 0x792C80) | S = max(A·corner_speed_in_ratio, B) | Ported (`Pace`, `corner_speed_ratio`) |
| Random brake-line mistakes (generateMistakes) | Gated off in shipped data | Inert in both |
| Difficulty scaling | `ai_driver_parameter_mappings.xml` by difficulty (Normal 0.606) | Ported (`Parameters::new`) |
| Lapping / team-mate yield, unsafe-retiree removal | Choreographer manager | Ported (yield); removal as the retire drive |

## 3. Remaining differences (not ported 1:1)

- **Race distance.** The original measures the field along the progress track; the port uses
  the AI racing line (AUTHORED). Gaps differ slightly on tracks whose AI line and progress line
  diverge.
- **Random numbers.** The original draws from the game's global RNG, whose stream depends on
  everything else that uses it; the port uses its own seeded MT19937. Event timing and car
  choice cannot match draw for draw.
- **Conveyor details.** The slot count and the place convention are AUTHORED; the threshold rule
  (own distance + U(0, max_sleep_distance), 0x4027B0) is ported as researched.
- **Choreographer lap length** (`+0x80→+0x70`) and participant status 4 are not traced.
- **Off-line assist side.** The original takes the inside assist when `lat·κ ≥ 0` (0x404C90,
  lat from 0x40AE90); the port's `grip_factor` takes it when `lateral·curvature < 0`. This
  agrees only if the port's lateral sign is the opposite of 0x40AE90's, which is not verified.
- **Redgate in traffic.** robustness's live Donington capture (FIRST_CORNER_LIVE.md §4): the
  original's back markers keep 25–28 m/s through Redgate, the port's slow to 18–22. The
  race-start grip factor matches the original, so the cause is elsewhere in the AI's braking
  in traffic (follow brake 0x77A1E0 is the first candidate); not traced here.
- **Redgate corner speed (F449 analysis, not ported).** The original's leader brakes ~18 m
  earlier with partial pressure (0.55–0.78), releases at 37 m/s and holds 37.0–37.6 m/s through
  Redgate with 0.45–0.6 throttle (live field02). The port's leader brakes later at 0.92–1.0, falls
  to ~35 and then sags to 33–34 m/s while its steering saws between −1 and +1. Causes found [V]:
  - The port's line-following curvature is `wrap(heading(ahead) − heading(here)) / look_ahead`
    from the racing-line **segment** directions, which are constant within a segment. In a
    hairpin it is exactly 0 for 10–20 ticks whenever the car and its look-ahead point are on one
    segment (1 309 of 6 000 solo ticks); the look-ahead then grows (0x779A20 reads |κ|), the
    target point jumps and the steering slams to ±1.
  - The original's 0x798E90 instead takes `acos(lateral_here · lateral_ahead)` (double, clamped
    to ±1) over the look-ahead, with the sign from `(point_here − point_ahead) · lateral_ahead`.
    The two line-point records are at track view +0x90 (point +0xB0, lateral +0xC0, offset point
    +0xD0, offset +0xE0) and +0x150 (+0x170 / +0x180 / +0x190); layout from 0x779C40 (offset point
    = point + offset × lateral). The +0x144 braking-table grip is `0x9AD500(vehicle)` (wheel grip
    × surface × ¼, × an aero/mass term), but the Lacetti's two brake tables (grip 2.0 / 2.2) give
    the same 45.4 m from 57 to 39 m/s, so it does not explain the gap.
  - Open: what the original's lateral is. With the port's laterals (raw gate normals, skewed
    against the racing line in a hairpin) the sign test flips inside one bend, so the original
    must build it differently, in 0x7878F0 (not yet decoded). Porting stops there; evidence in
    main checkout `local/driving/redgate-20261019/`.
- **GRID World names.** The career's `gw_%d_star` and `_h2h` names follow the same rule; the
  career session wires them when its races run.

## 4. Checks

`ai_pacing` unit test `settings_parse_and_print` covers the name rule (`rd_1_star` at 0.506,
`rd_3_star` at 0.706, `rd_6_star` at 1.0, the `touge` variant, item order and explicit names).
Full pre-push checks in PROJECT_LOG F439.

Evidence (private): `local/drift-ai/decomp/cu1/` (5D5530, 6AF7C0), main checkout.
