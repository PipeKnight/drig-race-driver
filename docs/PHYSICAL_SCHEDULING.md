# Physical-frame ownership and scheduling research

F190 joins whole selected AD6C40 group reconstruction to whole B77310/B7B850 solving, with a second sequence including preceding AD7040advance.400scenarios per variant/profile match; no whole-parent/live/parallel claim. See [integrated physical sequence](GROUPED_PHYSICAL_FRAME.md).

F189 verifies selected whole AD6C40 group reconstruction from supplied impact contacts: [contact groups and actual world ownership](CONTACT_GROUPS.md). Live paused state has three advance lists/two solve managers and both async flags enabled; physical32C is the world owner, while18 is the gated group root. This is not a measured live schedule. Earlier sections retain history.

F188 verifies the complete synchronous AD7040 world advance/list construction joined to supplied-contact physical solving,711scenarios per profile. See [world body advance](WORLD_BODY_ADVANCE.md). Whole world/contact production and active callbacks remain open; earlier sections retain history.

10 October 2026. This is **static dependency recovery**, not a measured runtime schedule. [F160](IMPACT_FRAME_CORRECTION.md) verifies a supplied physical-frame branch; authoritative contact production, active callbacks and its relationship to the car/wheel update remain open.

## Confirmed static chain

- File-backed relative branch B7BEED calls the verified whole B7B850. Complete disassembly from B7B850 through its return establishes a separate wrapper at **B7BEE0**, which forwards group and delta with `ret8` and preserves the manager receiver. Ghidra decompilation confirms the forwarding call. The first Ghidra reference lookup had no caller because this wrapper had not been defined as a function in the analysis project.
- The file-backed method table starts at **D42654**; its +18 slot at D4266C contains B7BEE0. Constructor **B7A9E0** calls base B77360 and installs D42654. D42614 is a nearby `PhysicsSoftBodyInitialise` string, **not** the table base; reading actual file words corrected that candidate. No source/class name inferred from the adjacent string is treated as runtime identity.
- **B77310** iterates manager+74's linked groups (next pointer group+1C), invokes receiver method-table+18 with each group and incoming delta, and writes the returned status at group+30. A nonzero status triggers each contact's virtual+0C through the group contact list. This error/sleep branch is not covered by simply looping the Rust frame API without its side effects.
- Direct original calls to B77310 occur at AD7173 inside **AD7110** and AD6B7D inside **AD6B70**. Constructor B7A9E0 is called twice from **AD8480**, with world-owned configuration. The meaning and ordering of these world job paths require the following targeted survey; no fixed tick or observed active branch is established yet.

The car side is already distinct: [vehicle update](VEHICLE_UPDATE.md) establishes wheel preparation scratch prediction and finalizer committed prediction. The finalizer first applies weighted impulses and saves the predicted pose, restores the initial state, applies raw impulses for final velocities, then commits the saved weighted pose. Connecting world/body and car/linked-state views must preserve that distinction rather than assuming one generic velocity-based pose update.

## World stage and job ownership

Successful survey `20261010T1117006259797Z/` and aligned `world-physical-job-aligned-20261010.json` establish AD7110's receiver as the enclosing world state. It waits on the collision producer through B187D0 with receiver world+18, drains world+7CC's queued groups through B24E70, calls AD6C40, calls AD6A10, then supplies **world+7C8** as delta. World+94 selects synchronous B77310 on the first manager in world+10, or AD6730 task dispatch. This establishes a field and static branch order, not who writes delta or which mode is live.

AD6B70 uses job+4 as delta and job+10 as physical-manager pointer; after B77310 it signals its embedded completion object at job+8 via virtual+14. These receiver/offsets come from aligned instructions; pseudocode omits some receivers. AD6730 iterates 0x14-byte jobs, skips empty manager+74 lists with completion signaling, writes incoming delta to job+4 and schedules nonempty managers.

AD6C40 reconstructs groups/contact lists and distributes them among managers. Its single-manager branch resets manager+74 and links groups via group+1C; the multi-manager branch sorts candidate groups by estimated cost and assigns each to the least-loaded manager. Before distribution it visits existing body links and supplied collision contacts; its point translation and AD6250 processing require independent full contracts. AD6A10 maintains ten countdown slots; their wider meaning remains provisional. These larger helpers are **not yet executed in a new whole-world comparison**.

Private survey `20261010T1118159315062Z/` contains AD6C40, AD6A10 and B187D0. Static caller survey `20261010T1119192600977Z/` locates AD7110 calls in **AD8A40** and **AD8D70**. Those parent methods and delta writers are the next ordering targets. Vehicle getters/setters A69630/A696F0/A69800/A69820/A69840/A69860 form the separate actual linked-state bridge to inspect before joining car and physical state. A job/frame being scheduled is not evidence that its vehicle callbacks or optional correction ran in a live race.

Successful bridge survey `20261010T1120026005940Z/` confirms the linked object receiver+8 points to the physical body: linear getters/setters copy physical+220, angular getters/setters copy physical+210, and pose getter copies the **published +1D0 matrix**, not the center-based +190 matrix. A696F0 copies a supplied pose and invokes whole B12710 with the second flag. This supports a concrete next integration target: published car pose → physical center/derived-state setter → physical frame → published state readback. B12710's full mutation/flag/shape/wakeup contract is not yet verified. Whole 9BD7F0/A03220 and full car job execution remain open; the bridge survey does not validate their other side effects.

## Evidence, tools and failed candidates

Private evidence under `local/behavior/`:

- `20261010T1107529180508Z/address-refs.tsv`: initial B7B850 lookup with no recorded callers.
- `outer-frame-direct-branch-candidates-20261010.json`: bounded raw E8/E9 candidates; these alone do not prove instruction boundaries.
- `outer-frame-caller-alignment-20261010.json`: guessed B7BE80 start decodes no instructions; not valid alignment evidence.
- `outer-frame-and-job-aligned-20261010.json`: known entry B7B850 through return and following B7BEE0 wrapper, including B7BEED call.
- `20261010T1111564283347Z/b7bee0.c.txt`: successful wrapper decompilation.
- `physical-frame-wrapper-pointers-20261010.json`, `physical-manager-method-table-20261010.json`, `physical-job-table-byte-candidates-20261010.json`: actual file words and candidate references. Raw occurrences require instruction/caller confirmation.
- `physical-job-constructor-neighborhood-20261010.json`, `physical-manager-constructor-entry-20261010.json`: aligned D42654 writes and B7A9E0 entry.
- `20261010T1115122015337Z/`: successful B7A9E0/B77360/B77BF0/B773D0 survey. Constructor configuration-to-manager offsets are recovered statically; allocation/reset paths have not been executed in a new comparison.
- `20261010T1115439798598Z/b77310.c.txt`, `20261010T1116148349578Z/address-refs.tsv` and `contexts.tsv`: group dispatcher, world callers and constructor callers.
- `physical-virtual-slot-20261010.log`: B00000–B90000 displacement survey exceeded its 2,048-result limit and was correctly rejected; partial output is not complete evidence.
- `20261010T1114394529617Z/displacement-refs.tsv`: narrowed B70000–B80000 survey completed with 280 results. Literal +18 matches from other objects remain candidates; the actual B77310 receiver/method-table chain establishes this use.

`tools/inspect_private_words.py` gains an opt-in `--byte-candidates` scan. It retains the old aligned scan, pins the same analysis SHA256, bounds each address to256 occurrences and output to ignored local/. Unaligned immediates at B78960/B7AA10 identified constructor/destructor table writes, subsequently confirmed by aligned instructions. This is research tooling, not game runtime code.

Ghidra12.1.4/JDK21.0.9, Capstone5.0.6 and Python3.12.14 reused. No installation or desktop input. Original analysis PE SHA256 remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. No new live scheduling or driving claim. Next recover the world job's complete ordering, delta source and body/vehicle handoff, then select a larger executable comparison target.
