# Career ("GRID World") of the original

18 October 2026, Claude Code (F416). How Race Driver: GRID's career works: regions, ranks and
licences, seasons, events and unlocks, money and reputation, the own team (cars, sponsors,
teammates), driver offers, Le Mans and the save. Static research only; the original was not run.
Port: `grid_tools::career` (§10).

Labels:
- **[V]** verified in a decompilation or disassembly of the hash-pinned analysis image
  (`local/compatibility/grid-unpacked.exe`, as in docs/RACE_RULES_RESEARCH.md), with addresses;
- **[D]** read from shipped data;
- **[I]** interpretation;
- **AUTHORED** a choice of this port, not the original's.

## Sources and tools

- Shipped data, decoded privately with the existing independent decoders (docs/RACE_RULES_RESEARCH.md):
  - `database.bin` dumped to one TSV per table: `local/career/data/tables/` (78 tables), in the main
    checkout's `local/`;
  - event inventory per tier: `local/career/data/inventory.txt` (script `local/career/scripts/inventory.py`);
  - `gameconfig/rewards.xml`, `season_leaderboards_setup.xml`, `ebay.xml`;
  - English text (`language_eng.lng`) and the front-end flow graph (`system/flow.bin`, decoded in
    `local/pause-menu/flow.xml`).
- grid.exe: Ghidra 12.1.4 headless, `-readOnly -noanalysis`, on private copies of the disciplines project
  (`local/career/ghidra-project`, `local/career/agent-{a,b,c,d}/`); Capstone 5.0.6. No tool was installed.
- Four parallel read-only research passes wrote detailed notes with every address:
  `local/career/notes/{REWARDS,PROGRESSION,TEAM,SAVE}.md`. This document is their summary; the notes
  hold the pseudocode references.
- Save evidence: the private backup of a freshly created profile from docs/AUTOSAVE_INPUT.md, copied to
  `local/career/agent-d/save-copy/` and only read.

Database fields are fetched by name at run time (0x832900 table, 0x81AAD0/0x8329C0 field getters), so
a field-name string's references find every use of a field [V]. `event.percentage_towards_completion`
and the `team_mate` table have no name string in the exe: they are **not read** [V by absence].

## 1. Structure

### 1.1 Regions [D]

`career_region`: 1 europe, 2 united_states, 3 japan, 4 international, 5 le_mans. Each region has
its own reputation (§3). International and Le Mans have no licences of their own; their reputation
only counts towards the total.

### 1.2 Ranks and licences [D, V]

`career_rank` (reputation_to_unlock): rookie 0, pro 80,000, expert 250,000, international 1,000,000.

| Rank | Europe | United States | Japan |
|---|---|---|---|
| rookie | Rookie (Euro R) | Rookie | Rookie |
| pro | Euro B Licence | ARL Pro Badge | J-Speed Super Two |
| expert | Euro A Licence | ARL National | J-Speed Super One |
| international | Global Race Permit (all regions) | | |

- Region rank (0x639150) [V]: the highest of rookie/pro/expert whose threshold the **region's**
  reputation reaches. Le Mans is always rookie. If the **sum of all five regions** reaches 1,000,000
  the rank is international in every region.
- Overall rank (0x6391B0) [V]: international by the total, else the best region rank.
- "[AMOUNT] required" (0x6392A0) [V]: threshold − region reputation for pro and expert; 1,000,000 −
  total for international.
- Licence countdown (profile +0x1694, 0x66B1D0) [V]: starts at 7. Each new regional pro or expert
  licence takes 1 off, and the Global Race Permit sets it to 0.
  - Garages: 1 at first, 2 after the first licence, 3 after the fourth (0x4DA640).
  - The International tier is first shown after the fourth licence (`itl_presented_check`) and opens at
    overall rank 3 (`itl_unlocked_check`).
  - [I] "Licences: [VALUE] / [TOTAL]" = 7 − countdown out of 7; the fact screen's producer was not found.
- Rookie licences in all three regions come with founding the team (0x66F640) [V].

### 1.3 Tiers [D]

`tier` of the main career (`career_id` 1):
- own-team tiers 1–9: europe/us/japan × rookie/pro/expert;
- 14 international;
- Le Mans 10–13: gt2 (rookie), gt1 (pro), lmp2 (expert), lmp1 (international);
- driver-offer tiers (`is_driver_offer_tier`):
  - per rank one per region, with `order_index` 0 US, 1 Europe, 2 Japan; the international offer
    tiers have region 4;
  - twelve Le Mans offer tiers 30–41 (three per rank).

The other `career` rows (garage, network, skipfe, testdrive, freerace, firstrace) are the front end's
other modes; each has one technical tier and event.

### 1.4 Events [D]

71 events (`local/career/data/inventory.txt`). Per own-team tier there are 3–7 events, one of them a
head-to-head:
- rookie events: 1–2 races, prize for first 15,000–35,000, reputation pot 40,000;
- pro events: 3 races, 85,000–200,000, pot 80,000;
- expert events: 1–4 races, 1–2 million, pot 200,000;
- international: five 5-race events, 3 million, pot 1,000,000, and the 10-million final head-to-head.

Event fields:
- `num_competitions` races are drawn from the event's `event_track_pool`, which is usually larger
  (bmw_si_tour: 2 races from 4 layouts) [D]. How the original draws them was not traced.
- `event_vehicle` lists the cars allowed.
- `event_sponsor_offer` has three offers per event (§5.1).
- `points_id` is 6 for every event: 20, 18, 16, 14, 12, 10, 8, 6, 5, 4, 3, 2.
- `money_string_id` chooses the prize split (§3.1).
- `reputation_pot` and `reputation_pot_min` (§3.2).
- `head_to_head`; `use_lemans_selection` (Le Mans Series events).

All disciplines appear: circuit series, Stock Car demolition, Drift Battle, Drift GP, Freestyle, Pro and
Midnight Touge, Downhill Drift.

## 2. Progress through the career

### 2.1 Arc 1: driving for other teams [V]

A new career has no team and no money. The player drives **driver offers** (§6). Their money builds up
until AddMoney (0x674610) sees the bank reach `career_global_settings.start_race_team` (40,000). Then:
- 40,000 is spent and the team is activated (0x66F640);
- rookie licences are granted in all three regions;
- the free `initial_vehicle_id` 43 (`bos`, a US muscle car listed at 55,000) is given (0x656100).

The flow then asks for a team name (`unlocked_second_arc`, while `Player_TeamName` is empty) and
offers the livery designer.

### 2.2 Arc 2: the own team

- Own-team events are entered with an owned car from the event's `event_vehicle` list.
- [I] A regional tier opens when the region's rank reaches the tier's rank, the International tier at
  overall rank 3. The availability code (0x6766B0, 0x675F40) was not fully traced.
- [I] Every event of an open tier is offered and can be repeated: the per-event records only
  accumulate, and no lockout was found. Repeats pay less reputation (§3.2).
- **Head-to-head** (0x656D40, after every event) [V]: a tier's head-to-head becomes available when
  every other event of the tier has its event-record field +0x64 set. [I] That field means "won"; its
  writer was not found.
- **Game completion** [V]: winning the head-to-head of an international-rank tier (`i_h2h`, event 78,
  10,000,000) sets profile +0x89 ("won_final_h2h"), which plays the completion video
  (`game_completed_check`).

### 2.3 Seasons [V]

A season is not a calendar.
- The season object (profile +0x1620) holds up to five (kind, id) entries. An entry is added when an own
  event or a driver offer finishes **while the team is active** (0x6427A0, 0x67BF2D).
- Season state (0x62C140):

  | Counted entries | Le Mans refused | State |
  |---|---|---|
  | fewer than 4 | – | running |
  | 4 | no | Le Mans pending: the Le Mans 24 Hours is offered |
  | 4 | yes | ended |
  | 5 | no | ended |

- End of season (0x4DBB20, 0x656760):
  - the leaderboard's season fields are cleared;
  - the three driver-offer slots are regenerated;
  - the season is copied to "previous" (+0x1650);
  - the season number (+0x1680, starts at 1) is incremented.
- No reset of money, reputation or event availability was found.
- While Le Mans is pending or the season has ended, finishing a regular driver offer does not refresh
  its slot (0x676DA4).
- The flow shows `season_end_info`, then `end_season`, then `season_start_info`.

### 2.4 Le Mans 24 Hours [V, D]

- It is offered once per season, when the state is "Le Mans pending" (`le_mans_entry_decision`, 0x58A200).
- The player can:
  - take a Le Mans driver offer (three extra slots from tiers 30–41, 0x651A10);
  - enter the own team with an owned car that has `vehicle.can_enter_le_mans`, in the class tier
    10–13;
  - refuse (0x4DBAD0), which ends the season.
- The Le Mans events 13–16 are single 8-lap races with `time_limit_seconds` 720 on Circuit de la Sarthe
  (docs/DISCIPLINES.md) and `reputation_pot_min` 5000 (100 elsewhere).
- Their reputation goes to the le_mans region, so it only counts towards the international total.
- `lemans_restrictions` builds the multi-class field (docs/SERIES.md); its career consumer was not
  traced.

## 3. Money and reputation of an event

The end-of-event dispatcher 0x67B910 runs only in career mode [V]. It calls:
- the head-to-head routine 0x6788D0;
- routine B 0x6781F0 when the game-mode virtual +0x98 (0x69B290) returns true: the current tier's
  region index (+0x48) is 4, `le_mans` [V]. Only tiers 10–13 are in that region, so B pays only the
  Le Mans 24 Hours events 13–16; the Le Mans Series events (31, 35, 50, 51) use A with class position;
- otherwise routine A 0x6779C0;
- driver-offer routines 0x678BD0 and 0x678EE0 for event kinds 1 and 2.

Routines A and B share the arithmetic below; B reads the pot directly, without the depletion of §3.2.
The differences are listed in §3.4.

### 3.1 Position and prize [V]

- **Reward position** (0-based): the player's **championship rank** over the event's races, not the
  last race.
  - Standings (0x66FF70): per race, points from table 6 (13th and lower score 0), plus races and wins.
  - Rank (0x6478E0): dense, so equal points share a rank.
  - In a multi-class event (routine B, or routine A with the flag at run +0x48) the position within
    the player's vehicle class is used instead (0x66BBA0).
- **Prize** (0x6395E0): `trunc(prize_money_for_first × money[pos])`, computed in f64. Places past the list
  pay 0 (0x826700).
  - `money` is `event_money_reward.money_string` of the event's `money_string_id`. It is chosen per
    event, not by the actual grid size: 4 "12_Car_grid", 5 "20_Car_grid", 6 "8_Car_grid",
    7 "6_Car_grid", 8 "h2h" [D].
  - The list for 6-car grids is `1.0,0.8, 0.6,0.4,0.2,0.1`, with a space [D].
- **Teammate's prize** (0x677D87): the team gets `trunc(prize(teammate pos) × (1 − winnings_percentage
  × 0.01))`. The teammate keeps their percentage of their own prize.
- **Team Win Bonus** (0x639760): `team_win_rewards` by the tier's rank (20,000 / 150,000 / 1,000,000 /
  3,000,000), paid when 0x660E90 returns 0.
  - 0x660E90 sums a per-car value per team within the player's class (player and teammate are one
    team) and ranks the teams.
  - [I] The value is the event points, and the bonus is paid when the player's team leads.
- **Money credited** (0x674610) = prize + teammate share + Team Win Bonus + sponsor money (§5.1) +
  the Flashback Bonus (career +0x68, the sum 0x652200 of the per-race records' +0x5C).
- **Flashback Bonus** [V, 20 October 2026; corrects the earlier "not paid"]:
  - The per-race recorder 0x66DB80 stores the award 0x6A22A0 of the race's flashback manager in the
    race record (+0x5C) when the player's result is final (result +0x11; [I] finished). There is no
    position or win condition.
  - 0x6A22A0: `maximum > 0 ? (maximum − used) × max_award / maximum : max_award` (i32 `imul`, `idiv`,
    truncated). With no flashbacks at all (Ninja, Pro Mode) the whole award is paid.
  - `max_award` is the `difficulty` row's `flashback_bonus` (3,000 / 12,000 / 30,000 / 60,000 by the
    tier's rank), `maximum` its `available_flashbacks` (0x6AF7C0). Example: rank 1, 4 flashbacks,
    1 used: 3 × 3,000 / 4 = 2,250.
  - All five reward routines sum it: A 0x677F98, B 0x6786E0, head-to-head 0x6788DD, driver offers
    0x678CEE and 0x678FFE (calls of 0x652200).
  - Not traced: where the per-race records (career +0x90, saved) are cleared between events, and a
    proof that career +0x30 is the manager 0x6AF7C0 sets up.
- **Bank screen** `ScreenRewardsBankAccount` (factory near 0x571400, constructor 0x5554F0, vtable
  0xCD4AE8, filler 0x555620) [V]: rows prize (+0x5C), sponsorship (0x652010), Team Win Bonus (+0x64),
  Flashback Bonus (+0x68), then the teammate's share (+0x60) only with a team mate. A driver offer
  shows wages (0x4E1B50) and objectives (0x652270) in the first two rows. Total is the rows' sum;
  balance and Season's Earnings count up from value − total.
- **Season's Earnings** [V]: the player's entry (id −2, field +4) of the money leaderboard at career
  +0xB0 (0x4CB040). Routine A (0x665560) and B (0x665890) add the event's credited money after each
  event; the season end (0x4DBB20 → 0x66E2E0 → 0x665BA0) zeroes every entry. The row shows only while
  0x4DA430 is true; otherwise the balance shown adds `start_race_team` (40,000) [I: the flag is "own
  team founded"].

### 3.2 Reputation [V]

```
pot   = routine A: the event's remaining pot (record 0x65F130, created at reputation_pot)
base  = trunc(pot × rep[pos])        rep = 0.5,0.4,0.3,0.2,0.17,0.14,0.12,0.1,0.08,0.06,0.05,0.04,0.03,0.02,0.01
bonus = floor(base × F + 0.5)        f32
gain  = base + bonus                 to the reputation of the event's tier region (0x66E1C0 -> 0x642850)
pot   = max(pot − base, reputation_pot_min)
```

Notes on the formula:
- **Repeats pay less.** bmw_si_tour pays 20,000 for the first win, then 10,000, then 5,000, and so on,
  never below 50 (half of the minimum pot of 100).
- Places 16 and lower get no reputation.
- Head-to-heads give no reputation.
- Negative amounts are ignored and there is no cap.

Bonus factor F (0x64A3F0, from the race settings) [V]:

```
F = 0.01 × ( 10 if locked to the head camera   (fixHeadCamera)
           + 20 if pro mode                    (proMode)
           +  5 each if traction control, steering assist or braking assist is off
           +  5 if manual gears                [I: the parser's `transmission` flag]
           + floor(difficulty.rep_bonus_factor × 100 + 0.5) )
```

- `difficulty` is the row for the tier's rank and the star level (Basic … Ninja = 0 … 5).
  `rep_bonus_factor` is −0.2, −0.1, 0, 0.1, 0.2, 0.3 by star level for every rank [D].
- The point values come from `rewards.xml` [D], with the same defaults in the constructor (0x677718).
- `realisticDamage` and `surfaceAssist` are loaded but **never read**. The menu says realistic damage
  "will add an additional reputation reward", but the code does not pay one.
- F is 0 in the non-career game modes 1, 3, 4, 5 and 6.

The total reputation shown ("Reputation: [REP] pts") is the sum of the five regions (0x62C4A0). The
teammate's reputation goes only to the leaderboards (0x665890).

Worked examples (first play):

| Event | Place | Money | Base reputation |
|---|---|---|---|
| bmw_si_tour (7) | 1st | 30,000 | 20,000 |
| | 2nd | 25,500 | 16,000 |
| | 5th | 18,000 | 6,800 |
| | 16th | 1,500 | 0 |
| lms_gt2 (31) | 1st | 150,000 | 40,000 |
| | 3rd | 112,500 | 24,000 |
| | 10th | 45,000 | 4,800 |

- A bmw_si_tour win at star 2 with all aids off and manual gears gives F = 0.2, so 24,000 reputation.
- With a teammate finishing 2nd at 20 %, the team also gets 20,400.

### 3.3 Head-to-head [V]

A win pays `prize_money_for_first` (500,000 to 10,000,000) and no reputation (0x6788D0). The
`head_to_head_rewards` car or its alternative money (`alt_reward_amount`) is granted on a branch through
0x65AD90 that was not traced.

### 3.4 Routine B and class position [V, 20 October 2026]

- **Selection.** The game virtual +0x98 is 0x69B290: the current tier's region index (tier +0x48, from
  table 0xDB4C4C: europe 0, united_states 1, japan 2, international 3, le_mans 4) equals 4. Only tiers
  10–13 are in `le_mans`, and their only events are 13–16, the four Le Mans 24 Hours (one race each).
  The Le Mans Series events 31, 35, 50 and 51 are in Europe tiers and use routine A.
- **A against B** (Capstone of 0x6779C0 and 0x6781F0):

  | | A | B |
  |---|---|---|
  | player position | class position if `use_lemans_selection`, else the dense rank | always class position |
  | team win | 0x660E90 in the class if `use_lemans_selection`, else the dispatcher's flag | always 0x660E90 in the class |
  | reputation pot | the event's remaining pot (§3.2) | the event's whole `reputation_pot` |
  | pot depletion | yes | none |
  | teammate position | as the player's | class position |
  | 0x65D440, head-to-head unlock 0x656D40 | called | not called |
  | leaderboard update | 0x665560 | 0x665890 |

  Prize, team-win amount, Flashback Bonus, sponsors, bonus factor and the money credit are shared.
- **Class position** (0x66BBA0):
  1. A driver's class is the `vehicle_class_id` of the vehicle of their `vehicle_livery` row (driver
     +0x20).
  2. Standings entries of another class are removed; the driver being placed always stays.
  3. The rest are sorted by points, descending; on equal points the player (driver type 2 [I]) goes
     first. Other ties are left to an unstable sort.
  4. The position is the driver's 0-based index, so the player's class position is the number of
     same-class drivers with more points.
- 0x660E90 applies the same class filter before summing points per team.

## 4. Cars [V, D]

- **Buying** (0x6569C0): `vehicle.purchase_price` from the bank, only if it is affordable.
- **Selling** (0x656B20): the fixed `sale_price`, with no wear factor.
- The cars allowed in an event are listed in `event_vehicle` [D].
- Not traced:
  - region garage restrictions on buying;
  - eBay (`ebay.xml`: auctions, mileage, crash chances, performance changes; event 22 is unlocked by
    eBay, `event_unlock`).

## 5. The team

### 5.1 Sponsors [V]

- **Slots:** one major slot (sponsor manager +0x44) and seven minor slots (+0x48).
- **Offers:** each event has three offers with `position_to_unlock` 1, 2 and 3 [D].
  - After an event, 0x673B20 → 0x699A20 → 0x67F000 unlocks every pool offer of that event with
    `pos < position_to_unlock`. A win brings all three, 2nd place two, 3rd place one; "Finish in the
    top three and sponsorship offers will start rolling in".
  - Each offer unlocks once. No limit on received offers was found.
- **Objectives** (0x639820 / 0x66E3A0, evaluators 0x655960, 0x655A10, 0x655AC0, 0x655B60):
  - finishing place: finished and `pos ≤ objective_details − 1`;
  - finish: finished;
  - the "no damage" variants also need the result's no-damage flag (+0x10). What sets that flag was
    not traced.
- **Pay** (0x66DB80, 0x652010, 0x642570): per race and per slot, `money_reward × (2 in the major slot)
  × (player met + teammate met)`.
  - The teammate's car carries the same sponsors and earns for them too ("pays double" is the major
    slot).
  - Multi-race events pay per race.
- `event.sponsor_id` and `minor_sponsor_id` are the event's trackside branding
  (`track_sponsors/%s/*.pssg`), not money.

### 5.2 Teammates [V]

- **Pool:** every `driver` row with `can_be_hired` (345 rows, 0x662850); the `team_mate` table is
  unused.
  - Each candidate has `signing_fee`, `winnings_percentage`, rank, region and the linked
    `ai_driver.Virtual_Performance` (0x649320).
  - Search (0x638A60) filters by fee range (`team_mate_fee_range`), percentage range and region
    (or Any).
- **Hiring** (0x6994E0): the signing fee is paid once and added to a spend total. Firing and replacing
  give no refund.
- [I] The teammate screen is forced right after the first licence and the second-garage presentation.
  The "not ready to hire" predicate was not found.

### 5.3 Liveries [D]

The livery designer is shown after the team is named:
- patterns, three colours and a number (`livery_pattern`, `livery_colour`, `livery_number`);
- per-car `vehicle_livery` rows have a `livery_price`.

No livery rule was traced.

## 6. Driver offers [V]

- **Slots:** three race-offer slots and three test-offer slots, one per region by `tier.order_index`.
  - Each slot shows the next uncompleted offer of the current rank in `driver_offer.order_index`
    order (0x651480, 0x650FA0).
  - A slot advances after its offer is driven (0x651920 / 0x651A10) and is regenerated at season end.
  - Whether a failed offer is retried or skipped was not determined.
- **Pay** (0x674940, 0x652270, 0x637980):
  - Appearance fee: `per_race_wage × driver_offer_track rows`, for race offers.
  - Objective bonus: `objective_money_bonus` and `objective_reputation_bonus` when the objective is met.
  - The reputation goes to the offer tier's region.
  - [I] No event prize or reputation pot is paid for an offer race; the real (non-editor) path was not
    settled.
- **Objectives:**
  - finishing place (Le Mans: class position): `pos ≤ place − 1`;
  - no damage;
  - beat rival (0x6556B0): finish ahead of every car of `objective_team_id`;
  - time trial (0x655D20): lap faster than `target_time_secs[star level]`;
  - speed test (0x655C80): top speed ≥ `target_speed_mph[star level]`.
  - The targets are six comma-separated values, one per difficulty star level [D].

## 7. Leaderboards [V arithmetic, I meaning]

`season_leaderboards_setup.xml` [D] drives the AI teams' season earnings (0x665C50):

```
t = (N − leaderboard_rank) / N
s = t^bias
E = t^(bot + (top − bot)·s) · 0.25 · (max − min) + 0.25 · min
E is multiplied by 0.8 when 0x62CEB0 is true
R = (botRand + (topRand − botRand)·s) · E
```

E is the target earnings and R their random spread. N, the 0.25 factor and the condition are not
resolved. The player's team is ranked by its accumulated earnings [I].

## 8. Career-only race rules

What a career race adds to the race rules of docs/RACE_RULES_RESEARCH.md:
- the event's races are a championship with points table 6, and rewards follow the championship rank
  (§3.1);
- race settings change reputation (§3.2);
- pro mode forbids restarting from the pause menu (`help_info_event_options_pro_mode_toggle`) [D];
- flashbacks are limited by `difficulty.available_flashbacks` (5 … 0 by star level) [D,
  docs/FLASHBACK.md];
- a teammate races in the field;
- sponsor objectives (with no-damage variants) are checked per race;
- the pause menu shows sponsor objectives (`ScreenPauseSponsorObjectives`);
- "Retire" can retire from the race or the whole event;
- [D] the player's grid slot comes from `event_track_pool.player_grid_position_base/range`
  (docs/RACE_RULES_RESEARCH.md §5);
- named drivers have their own slots (`event_driver_grid_position`), and the career grid is filled by
  0x5C7D40 (docs/SERIES.md).

## 9. Save format

### 9.1 Container [V, D]

Every save unit is one file in `Documents\Codemasters\GRID\savegame\Autosave0\`:

```
u32 checksum   wrapping sum of the u32 words from 0x10 over the unit's capacity (0x7F07D0)
u32 version    0x1FD
u64 profile id the PROFILE unit's profile_id
N × { u32 length (0x400, or 0 if never written), 1024 bytes }   N = capacity / 1024, rounded up
```

- The 1024-byte chunks are encrypted with **AES-256-ECB** under one static key for every unit and every
  installation.
  - The key is an ASCII hex string in the exe at 0xD04C00. It is not reproduced here; the private
    tools in `local/career/agent-d/` use it.
  - Encrypting zeros gives the repeated block seen in the files [D].
- There is no MAC.
- Unit sizes are fixed capacities, so files do not grow.
- The tail of the last chunk is stale memory.
- The private tools decrypt all 15 units of the new-profile backup, and re-encrypt them byte for byte
  identically [D].

### 9.2 File names and units [V, D]

- A file's name is its unit name upper-cased and shifted letter by letter with the key "YGPNELSQZIK"
  (0x7F0D00).
- `CAREER MANAGER SDU NAME` holds the career: `main_career` (0x6501B0) and `garage_career`.
- The other units:
  - `RACEHISTORY` (statistics, the `fact_*` screens);
  - `EBAY_MANAGER_SAVE` (eBay sale listings, 0x60 bytes each, and the bought cars' records; §13);
  - `REWARDS_SAVE` (season leaderboards);
  - `UNLOCKS` (52 vehicle and 15 bonus records);
  - `PROFILE` (a `<PlayerProfile>` XML of typed items: settings, livery, names, once-only presentation
    flags such as `done_first_career_race`, `presented_first_sponsor`, `unlocked_international`, and
    `PercentageComplete`, `RacesWon`, `RacesLost`);
  - `SYSTEMFILE` (slot title "Reputation: 0 pts, Season: 1");
  - `INPUTDATA` (controls);
  - seven small counter units.

### 9.3 Career unit, new profile (0x111 bytes) [V, D]

| Offset | Field |
|---|---|
| 0x000 | teammates: withdrawn-driver list, hired-teammate list, a map of int pairs, total signing fees |
| 0x010 | sponsors: received offers {id, new flag}, major {id, value}, 7 minor {id, value}, a progress map |
| 0x058 | per-region driver-offer board: 3 ints + 2 × 3 `order_index` references |
| 0x07C | 4 × 2 per-region flag bytes |
| 0x084 | **money** (i32) |
| 0x088 | **reputation** i32[5] (europe, united_states, japan, international, le_mans) |
| 0x09C | current and previous season (48 bytes each: 5 (kind, id) pairs, count, Le Mans refused) [I layout from §2.3] |
| 0x0FC | **season number** |
| 0x100 | pending rank promotion (rank, region, licence countdown 7) |
| 0x10C | event-in-progress flag and optional event snapshot |
| 0x10D | garage_career field |

The reputation-pot records of §3.2 (0x65F130) live in the career object, but their save path was not
identified. Only a new profile was available, so no non-empty career field was checked.

### 9.4 The port's save [AUTHORED]

The port does not reuse the container (`grid_tools::career::save`). It writes:
- a versioned UTF-8 document (`grid-career 1`);
- one `key value…` line per field, referring to database ids;
- an FNV-1a 64 checksum line at the end;
- atomically (a temporary file, then rename).

It holds every field the rules use:
- money, the five reputations, the season number;
- the team (founded, name), cars, teammate and signing fees;
- received and placed sponsors, the reputation pots;
- completed offers and best event results;
- the current and previous seasons, the Le Mans refusal;
- open head-to-heads, game completion, races won and lost.

## 10. Port (F416)

`grid_tools::career`:
- **`data::Catalogue`** reads the career tables of `database.bin` and `rewards.xml` (ranks, tiers,
  events with their track pools, cars and sponsor offers, vehicles, sponsors, driver offers with races,
  teammates, difficulty, points, team-win and head-to-head rewards, global settings).
- **`rules`** has the traced formulas: `prize`, `reputation_base`, `bonus_factor`, `reputation_bonus`,
  `standings` (dense ranks), the sponsor objectives, unlocks and per-race pay, `teammate_share`,
  `player_team_first` [I], the driver-offer objectives and `appearance_fee`.
- **`state::Career`** holds money, regional reputation, ranks, licences and garages, seasons, the team,
  cars (buy and sell), the teammate and sponsors. Results are applied with:
  - `apply_event`: routine A, including pot depletion, sponsor unlocks, head-to-head availability and
    game completion;
  - `apply_driver_offer`: the fee and bonus. Money from offers founds the team at 40,000.
- **`save`**: §9.4.

Tests:
- worked examples on a small catalogue: prize and reputation, bonus factor, tied standings, pot
  depletion, sponsor and teammate money, founding, licences, seasons, head-to-head, cars, save round
  trip and tampering;
- two ignored tests on the purchased data: the catalogue, and a rookie career from driver offers to a
  won bmw_si_tour.

AUTHORED or [I] in the port:
- [I] tier availability and repeatable events;
- equal-points order in the standings;
- [I] the team-win test;
- the offer that founds the team does not count in the season (ordering not traced);
- [I] head-to-head "won";
- no routine B, Flashback Bonus or eBay;
- the race draw from the event pool and the AI field of a career event are left to the app.

## 11. Open questions

1. Routine A vs B (game-mode virtual +0x98) and the class-position flag at run +0x48.
2. The availability code of tiers and events (0x6766B0, 0x675F40); the "Licences" fact formula.
3. The writer of event record +0x64 (head-to-head unlock); the `head_to_head_rewards` car branch.
4. The no-damage flag threshold; 0x652200 (record +0x5C) in the event money.
5. Driver-offer slot rotation after a failed offer; whether offer races also pay the event pot.
6. Season leaderboard N, the 0.25 factor and 0x62CEB0; the player team's season rank.
7. Answered in §12: the race draw and the career field. Still open there: whether the drawn race
   list is saved; the Le Mans 24 Hours flag (+0x98); the tie order of the grid ranking 0x5B6340; the
   player's car and livery in an own-team field.
8. The car record in `EBAY_MANAGER_SAVE`, `RACEHISTORY` statistics, `REWARDS_SAVE`, and where the
   reputation-pot records are saved.
9. Live check (owner-driven, not automated): one rookie event's bank and reputation screens against
   §3, with known settings.

## 12. Race draw and career field (19 October 2026)

Static research (agent pass, `local/career/notes/DRAW_FIELD.md` with every address and worked examples).

### 12.1 Race draw [V 0x67BC80, 0x664E30, 0x66D800]

- `N` races: `event.num_competitions`; for a driver offer, its number of `driver_offer_track` rows
  (0x637980).
- The event's pool rows (table order) are sorted, stably (pools have at most 11 rows, so only the
  insertion sort of std::sort runs):
  - an event never completed (profile event record +0x60 == 0): by `order_index`;
  - an event completed before, and every driver offer: by the profile's race count on each
    `track_model` (incremented after every race, 0x61017D).
- Pick: first every entry whose venue (`track_id`) is not taken yet, then fill from the sorted list.
- No RNG. The list is built when the event is entered (0x67BE50).
- bmw_si_tour on a first entry: Nürburgring Sprint (3 laps), then Nürburgring GP reverse (2 laps).
  All four layouts share one venue.

### 12.2 Career field [V 0x676A80, 0x673F20, 0x663D40, 0x66D350]

- One AI car per `event_driver` row of the event, in table order (driver, `vehicle_livery_id`). The
  car count is the row count (mostly 12; 20 for events 7, 28, 29, 34).
- Each AI car's team is `driver.team_id` (0x62AC70). The beat-rival objective and the Team Win
  Bonus use it.
- **The player's team takes over a team's cars:**
  - The candidate teams are those with at least as many eligible cars as the group (player, plus the
    team mate if hired), in team id order, excluding `key` and `primary_key` team types.
  - One is drawn with the field generator: `cand[rng(0, k)]`, MT19937 seeded in milliseconds.
  - The player overwrites that team's lowest-index car, and the team mate the next one.
- **Legend substitution:** if an event driver is the player's hired team mate, a random `event_legend`
  driver of the event takes the seat and keeps the original driver's team.
- **Ravenwest guest cars** [I meaning]: once a region has a finished event, Ravenwest's two drivers
  (team 1, `primary_key`) take over a team's cars in `_hero` liveries. This lasts until an event with
  them in the field is completed.
- **Race offers** (0x674020): the offer event's `event_driver` field. The player takes the offer's
  livery and overwrites the lowest-index car of `driver_offer.team_id` (key teams allowed). That
  team's other cars are the player's team mates, and the own hired team mate does not race.
- **Other formats:**
  - Test offers: the player alone.
  - Head-to-head: the rows plus the player.
  - `use_lemans_selection` events and offers: `lemans_restrictions` rows for the player's vehicle
    (0x663860).
- **Grid** (0x5C7D40):
  - the player's slot from the current layout's pool row (§RACE_RULES 5);
  - named drivers from `event_driver_grid_position` (same formula);
  - everyone else ranked into the free slots by
    `n − 1 − trunc((n − 1) · clamp(driver.skill + U(−0.2, 0.2), 0, 1))`.

## 13. eBay Motors used-car market (19 October 2026)

Static analysis of the eBay manager (ctor 0x675210, item vtable 0xCEA68C); private notes and
addresses in the main checkout's `local/career/notes/EBAY.md` (Ghidra 12.1.4 headless, read-only, on
the hash-pinned image; Capstone checks of the float code). Ported as `grid_tools::career::ebay`.

### 13.1 Settings and generators [V, D]
- `gameconfig/ebay.xml` (`EbaySettings`, parser 0x631E60) holds 32 values [D]. Among them:
  3 cars per model, 10..100 events, crash chances 3 % / 40 %, Buy It Now 88..100 %, auction
  length 4, value 70..130 % with noise −10..0 %, bids from 70 % in steps of 10 %, and
  0.33 / 3.0 / 500.0 "for one percent".
- `winningChancePercentage` is parsed but never read [V by absence].
- Two MT19937 generators, seeded by the original's rule (`mt[0] = seed | 1`, `mt[i] = mt[i−1]·69069`; `series::Mt`).
  - #1 makes the offers, the value noise and the buyers. #2 makes only the per-model seeds.
  - Reset seeds: 0x6529A9 and 0x221C63.
- `int(lo, hi)` = `lo + genrand % (hi − lo)`; `float(lo, hi)` = `lo + genrand·2⁻³²·(hi − lo)`.

### 13.2 Buying: fixed-price offers [V]
- There is no bidding for the buyer.
- Per-model seeds (0x65E9E0): `int(0, 0xFFFF)` per `vehicle` row in table order. They are redrawn:
  - after each career event (hook 0x587580);
  - after each eBay purchase;
  - at load, by replaying the saved refresh count from the reset seed.
- Showing a model's offers (0x668BB0) reseeds #1 with the model's seed and generates 3 cars
  (0x653360). The draws, in order:
  1. a seller among the `can_be_hired` drivers;
  2. N = 10..100 events;
  3. miles = U[250, 499]·N;
  4. event wins = trunc(U(0, 0.5)·N);
  5. race wins = U[1, 5]·event wins;
  6. per event, a major crash below 3 %, capped at N − wins;
  7. per event, a minor crash below 40 % (unused).
- Performance %, with f = wins / N:
  - f > 0.3: `1 + (f − 0.3)·10`, at most 5;
  - f < 0.2: `−(1 + (0.2 − f)·9/0.19)`, at most −10;
  - otherwise 0.
- Damageability %, with g = major / N, when g > 0.01: `1 + (g − 0.01)·(10 − 0.01)/0.49`, at most 10.
  The numerator `max − minMajorDamageFraction` is the original's own, kept.
- Price: `trunc(purchase_price · f32((100 − (miles − 500)·12/49500 + 0.33·wins − 3·major)·0.01))`,
  floored to the currency step (0x639A80: 50000 / gcd of the `currency` rates ×100 = 1000).
- Buying (0x65F540) is refused for a model already owned or a price above the bank. Otherwise
  the car joins the garage with its history (odometer in metres = miles·1609.344), the
  {model, performance, damageability} record is kept, and the seeds are redrawn.

### 13.3 Selling: an auction over career events [V]
- The player sets the duration (at most 4), a start price and a fixed price (0x65F390).
- Value (0x631B60) = `trunc((clamp(100 + race wins·0.6/0.33 − major·10/3 − miles·0.5/500, 70, 130) + U(−10, 0))
  · purchase_price·0.01f)`. The value is redrawn every event.
- Each event (0x64E760) `bid = trunc(value·(70 + 10k)·0.01f)` for k = 0..duration − 1, chop rounding
  (control word | 0xC00), floored to 1000.
  - Below the start price there is no bid.
  - At or above the fixed price the car sells at the fixed price.
  - At the end it sells at the last bid if the start price was ever reached, else it stays unsold.
  - Each step draws a random buyer.
- States 1 bid in progress, 2 fixed price met, 3 sold, 4 ended unsold, 5 no bids. Each has an
  `ebay_msg_*` popup.
- Completion (0x655F60) removes the car and its record, pays the money, and counts sold and earned
  (`fact_ebay_earnings`).
- Saved (`EBAY_MANAGER_SAVE`): sold count, spent, earned, refresh count, the bought cars' records
  and the active listings (the 0x60-byte list is the listings, not garage cars as §9 said).

### 13.4 Not traced / not ported
- What the bought car's percentages change: race setup 0x5F8940 writes `(100 + perf)/100` to
  physics +0x188C and `dmg/100` to damage model +0x434 (meanings unknown). The port keeps the
  record but does not apply it.
- Which models the eBay list shows (unlock, licence, region filters), the event-22 eBay unlock,
  whether the hook runs per race or per event (the port: per event and per driver offer), and
  how the garage car's history grows with races (the port: only eBay cars carry one).
