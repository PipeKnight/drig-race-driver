# Physical contact handoff — 10 October 2026

Static analysis locates the serial callback/consumer inside the larger physical update. This is a map of dependencies, not verification of a whole physical frame or live scheduling.

The outer update at B7B850 gathers contacts into four ordered pools, calls preparation manager B78530, runs the physical solver B7ACC0 and, on its successful branch, invokes impact manager B78710. The latter contains F151's verified serial callback/consumer. An optional correction pass B7B5E0 follows when manager +F4 is nonzero and +F0 exceeds global D85A50; body publication helpers follow it. The precise meanings of these enabling fields and live cadence remain unresolved.

Contacts with type byte +0C equal to zero and a nonnull second body go to the two-body pools; other contacts use the single-body pools. Byte +0F selects the impact-eligible pool within each category. The filtered gathering branch skips contact +0D; the all-contact branch does not. Do not substitute the disabled byte +0E for either flag.

The optional pass calls B0CE10 for impact two-body contacts and B0CEC0 for impact single-body contacts. Both read accumulated contact +38, which F151's consumer can modify. Thus there is a concrete downstream physical consumer of that accumulator. Whether the optional pass ran in a particular live frame has not been captured.

The main physical solver remains substantial: constraint construction (including B0D1B0/B0E040), solving, writeback, force/velocity integration and body publication are unimplemented as a whole. Verifying the small correction helpers will close one dependency; it will not establish drivable physics.

## Private evidence and research-tool correction

- Caller references: `local/behavior/20261010T0311427235400Z/`.
- Outer update: `local/behavior/20261010T0312197636405Z/b7b850.c.txt`.
- Solver and optional pass: `local/behavior/20261010T0313526540738Z/`.
- Correction and initial constraint builders: `local/behavior/20261010T0314187737909Z/`.
- Aligned correction instructions: `local/behavior/contact-bias-instructions-20261010.json`.

A broad displacement survey exceeded its 2,048-reference limit. Ghidra's headless process still returned zero and left a partial TSV; the existing PowerShell wrapper incorrectly accepted the file's existence as completion. That partial output is retained at `local/behavior/20261010T0312011883507Z/` and must not support complete-survey claims.

The wrapper now requires the script's completion marker and absence of `REPORT SCRIPT ERROR`, retaining logs and partial evidence on failure. An optional function-entry range narrows the displacement scan without changing its bound. The B70000–B80000 range completed with 205 references at `local/behavior/20261010T0313165037578Z/`. Repeating the known over-limit broad scan with the corrected wrapper rejects it: `local/behavior/field-survey-guard-check-20261010.log`. Ghidra 12.1.4 and JDK 21.0.9 were reused; nothing installed.

The existing store-only survey also completes through the new guard: `local/behavior/field-store-completion-check-20261010.log`. Complete single-body helper instructions, including its return, are retained in `local/behavior/contact-bias-tail-20261010.json`.

## Unattended execution state

The guarded restart produced private GRID PID9268, start UTC03:08:48.4528148Z, same b1da1b2d… reference hash, with user settings/profile snapshots. Computer Use could inspect the title prompt but could not activate the window, including a fresh selection/retry. The AHK foreground guard refused the menu confirmation before sending input at03:10:42UTC. No new race or live ownership capture is claimed. Continue offline while the owner sleeps.

Bounded thread continuation `grid-overnight-continuation` runs every 30 minutes until 10 October 2026 07:30UTC. It prioritizes substantive first-lap dependencies, Markdown records and focused commits, keeps proprietary evidence private, and stops or pauses when the owner returns or the window ends. This is a temporary continuation schedule, not a project completion promise.
