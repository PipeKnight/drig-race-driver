# Serial geometric pair scheduling (F222)

12 October2026, Europe/Kiev. Whole original serial pair assignment, worker dispatch and finish now run against actual Lacetti/Donington geometry: **1,364 original/repeated cases /64,689,104 complete output bytes per run**, producing2,888 contacts in726 positive cases. Both ownership orders produce the exact same contact arenas as [F221 direct generation](PACKED_MESH_CONTACTS.md). The static-collision gate disables generation in both orders.26 rejection/existing-output guards pass.

The complete audit checks4,145 output files,245 separately executed zero-capacity references, reconstructed requests and current hashes. A rebuilt default probe replays all1,417 F221 cases and matches all69,092,472 earlier output bytes. This validates the original serial scheduling boundary with an existing pair. It does not execute moving broad-phase publication, body advance/group/solve in one native frame, sleeping/inactive-group rescheduling, asynchronous workers or a Rust geometric generator.

## Frame order and owned scheduling

Aligned AD8D70 instructions confirm a physical world and a separate collision producer: world18 points to the producer. After AD7040 body advance and AD8070 cursor maintenance, the original calls B18E20 on the producer to update moving bounds/pairs, then B18300 to assign/generate primary contacts. B184F0 handles affected inactive groups; AD7110 groups and solves, and AD8130 finishes publication. The current probe executes the scheduling segment B152E0/B18300/B184F0/B187D0 with a supplied pair; it does not silently skip the preceding broad-phase work while claiming the whole frame.

B18300 resets the prior queued interaction state, clears worker heads and invokes B38EE0 to reset used count/contact cursor. It walks body328 shape links, shapeD0/D4 pair lists, and assigns pair44 worker chains and pair40 producer chains. With serial934=0 it runs B39730, which invokes B39500 for each worker pair. A body paired with static geometry is eligible only when its416 gate is enabled. Reverse ownership uses the actual11/7 entry8B0 with swap1; Ghidra B18E70 confirms the same AEA1F0/ADE540/AE4700 functions as7/11.

B184F0 clears worker assignment heads again before considering inactive groups. Therefore a cleared final worker8 does not mean primary generation failed. The probe observes the actual head immediately after B18300, preserves it in its output, and verifies that it equals the owned pair when eligible and zero when disabled. The final head is zero, while pair contact records and the producer930 queue remain available to later grouping. No affected inactive group exists in this selected active-body scope, so the returned group head and C24 flag remain zero; B187D0 does not wait.

## Fixtures and output checks

All245 Donington chunks receive the F221 height-zero placement, with static gate Off/On and forward/reverse pair ownership:980 cases. The192 F221 capacity controls receive both eligible ownership orders:384 cases. Current installed source, source Lacetti seed and original analysis PE provenance remain pinned through F221. No new capture or proprietary material is published.

PC02 extends the PC01 header with one u32 before its seed: bit0 supplies body416, bit1 reverses pair ownership; other bits are rejected. The source cold body's400=-1 is explicitly changed to active selector0 for this scope. One owned shape link/pair graph, worker pointer array and C40-byte producer replace the direct A80 dispatch-only root. This is prepared ownership, not original registration or native activation.

The output retains the complete F221 prefix and arrays, followed by producerA80..C40, worker pointer identity, affected-group head and observed pre-finish assignment head:1CC additional bytes. All canary, identity, complete PE-section comparison, lazy-page restoration and CREATE_NEW checks remain in force. The body is unchanged during the actual scheduling invocation.

The validator checks complete queue/worker/producer-tail state, both shape/pair backlinks, active/static gates and all independently checked F221 geometry fields. It removes only explicit ownership/order/counter/activation differences to compare every remaining byte with direct generation. Disabled cases compare against whole original direct zero-capacity executions, preserving complete original slot/scratch/triangle/cache templates. Normalizing the authored activation/static-gate bytes for that comparison is explicit; it is not a claim that active and cold bodies have identical native ownership.

No Rust physics code was changed. F221's solver proof remains tied to its explicit cold normalization. Matching contact records do not by themselves establish a complete retained runtime frame or a driven wall collision.

## Evidence, versions and corrections

Private final reports: `local/behavior/packed-contact-schedule-20261012-final-01/validation.json` and `packed-contact-schedule-regression-20261012-final-01/validation.json`. Audit: `packed-contact-schedule-audit-20261012-final-01.json`. Scheduled probe SHA256 is `c06fb64c7537dc019026f2b85632f175e1f81ee3c147167e250096bb60d6baa6`; rebuilt default regression probe is `de2b5aaf9c6a37f02cd2b54b30f6cb4dbaf36b992a7a48f9b1883a807866c2b9`. Build log `packed-contact-schedule-build-20261012-draft-04.txt` builds both. The pinned F221 executable was preserved separately and still passes its current-hash audit requirements.

Draft01 caught an access violation at original B185E7 reading11: the cold source body retained400=-1 and a canonical group link, while the probe had not prepared an inactive group. This run restored the reference image and emitted no output. Use the explicit active selector0 scope instead of attributing that fault to the game. The same draft's disabled cases stopped during pointer normalization because the worker identity lacked an owned region. Adding that region corrected the checker.

Draft02 generated contacts but its assertion expected worker8 to survive B184F0. Actual finish behavior clears it. Draft03 captures the head before finish and verifies both stages; all four controls then pass, with eligible contact arenas identical to direct execution. The declared output suffix is1CC bytes; no guessed tail bytes are omitted. These failures and outputs remain private. No approval was bypassed and no owner help was needed.

Ghidra12.1.4/JDK21.0.9, Capstone5.0.6, Python3.12.14, MSVC14.44.35207/DevPrompt17.14.41 and SDK10.0.26100.0 were reused. Fresh schedule decompilation is under `local/behavior/20261012T0950504538884Z/`, aligned calls under `fresh-geometry-schedule-calls-20261012.json`; prior0422576933384Z worker/body-pair evidence and B18E70 table evidence were reused. Two new Python AST checks and Git whitespace validation pass; no new Rust test run is implied. All130 checked Steam resources and the original executable remain unchanged at10:05:36.245700UTC: `local/scene/preservation-20261012-contact-schedule.json`.

Next: implement the actual triangle/mesh contact generator in Rust and connect moving-shape publication plus fresh queries to the retained vehicle tick. Existing packed ray queries do not replace triangle/mesh collision detection. Keep callback, sleeping/group wake-up and full asynchronous scope separate until executed and verified.
