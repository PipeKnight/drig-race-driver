# Generated contact groups joined to complete physical solving

F196 extends this ordinary-contact baseline with verified [shape classification
and generated wheel-contact handoff](WHEEL_PHYSICAL_HANDOFF.md), retaining special
flags and null second physical ownership through complete solving.

11 October 2026, F190. Independent Rust now joins [contact-group reconstruction](CONTACT_GROUPS.md) to whole serial group dispatch and complete physical solving. A second API also performs the verified world body-advance stage before this sequence. This closes the previous assumption that the physical solver receives one preassembled group.

## Integrated behavior

`grouped_physical_frame::calculate` reconstructs groups from a supplied contiguous impact pool, then visits the actual manager group order. Each group has its own ordered bodies, ordered contacts and local solver indices. The implementation retains global body/contact identities while gathering and publishing each local frame.

One shared manager carries row storage, workspace, inverse properties, material cache and previous span ends between groups. These arrays use local indices; an array prefix overwritten by the last group cannot be mistaken for world-indexed body storage. Complete body/contact buffers remain indexed by their owned world identities. Per-group results are committed only inside a private result; failure publishes nothing.

`calculate_tick` first reverses the active list into the original update order, advances each body and publishes center/object/derived matrices. It retains previous center/object poses and clears the advance response vector. It then reconstructs contacts/groups and solves them. Supplied first contact points are **relative offsets translated by the advanced first-body center**; geometry is not detected by this API. The stored update chain survives the subsequent active-generation switch.

The selected boundary has cold all-active bodies, selector0/head body0 in the private diagnostic, ordinary impact contacts, positive iterations, row capacity at least four times the contact count and the original maximum-float sleep duration that prevents sleeping. Shapes, existing body links, registered callbacks, waking/sleeping transitions, special-body normalization, persistent pools and parallel assignment are excluded. The typed APIs preserve a supplied complete active permutation; the isolated grouper separately verifies both selectors and arbitrary first heads.

## Original comparison

The native adapter calls whole unchanged `AD6C40 → B77310 → B7BEE0 → B7B850`, using the mapped original manager method table. The tick variant first calls whole synchronous `AD7040 → AD6C00 → B13E10` with an actual owned world backlink. These are recovered original **stage calls composed in order**, not a claim to have executed whole enclosing`AD8D70`. Collision generation and intervening broader-world stages remain external.

All pointers bind to owned buffers or the pinned original method table. The adapter checks world/producer retention, manager configuration, final per-group counters, body/center/derived backlinks, source body/material identities, group statuses, update links and every normalized row reference. Because later groups overwrite only a row prefix, friction-reference normalization tracks the last writer of every row across all groups, including retained rows from earlier groups. Code is unchanged, no imports/entry point/TLS execute, five original lazy-data pages restore their contents/protections, and rejected partial outputs are removed.

Both debug and release compare **400 scenarios /1,817bodies /1,162groups /22,601,160complete observable bytes per variant per profile**, original/repeat/Rust exact. The frame and tick variants each include357multi-body groups,277cases joining components,40empty-contact cases,3,356single-body contacts and2,749two-body contacts.1,783body velocities change in each variant;829poses change without advance and all1,817with advance.

Sixteen independent graph/physical checks pass for the frame; eighteen pass for the tick. They check connected-component partition, ordered ownership/counts/local indices, manager/dirty lists, generation switch, contact point translation and retention, force consumption and history publication; the tick also checks update-list reversal and advance response clearing. Each variant/profile has28ordinary malformed outcomes (14native+14Rust), including cycles, missing/foreign bodies, old ownership, sleeping configuration, insufficient capacity, zero iterations, shapes and nonfinite basis. No native fault or numerical mismatch occurred.

A Rust test fails in a later group's pose publication after an earlier group has already changed velocity/consumed force. Both frame and tick APIs preserve the complete input world and shared manager on failure. All152librarytests, formatting, warnings-denied all-target Clippy and Python AST checks pass. Fresh rebuilds also reproduce all711F188world-advance cases in both profiles, including their F162numerical baseline and PFR3regressions,22malformed outcomes and eight semantic checks per profile.

## Failures, evidence and remaining work

The first native draft stopped ordinarily after two scenarios with retention error10: the previous single-group adapter expected manager counters`A8..B8` to remain equal to the total input pool. Multiple groups legitimately replace them with the last group's counts. The adapter now permits those documented writes and independently checks them against the final group. No result was accepted from that failed run. Clippy rejected literal`4/4` array indices; they were replaced with direct indices before final verification.

Private final evidence: `local/behavior/grouped-physical-{frame,tick}-{debug,release}-20261011-final-01/validation.json`; `world-physical-tick-{debug,release}-20261011-f190-regression-01/validation.json`; original requests/outputs, guards and `grouped-physical-final-audit-20261011.json`. Failed/smaller drafts remain private. The current native/Rust hashes and unchanged analysis-image hash are included in each final report/audit.

Python3.12.14, Rust1.99.0 and MSVC14.44.35207/SDK10.0.26100.0 strict x86/SSE2 reused; no installation. Original130Steamresources and executable unchanged at09:57:15.134425UTC (`local/scene/preservation-20261011-grouped-physical.json`). No game input or settings changes. The expired overnight automation remains paused.

This is an integrated selected physical sequence, not a drivable game, a carried persistent-world loop, a geometric collision detector or a measured live asynchronous schedule. Next connect the real car update/publication and handling state to this world sequence, generate authoritative contacts, complete source-only initialization and drive the resulting carried state through Bevy. Project completion estimates remain unchanged.
