# Linked vehicle synchronization and carried publication — F181

F187 correction: the complete synchronous `A09A80` final local masks clear linear X, angular Y and position X/Z; the earlier W-only interpretation was incorrect. `A03220` itself still clears velocity W. See [whole wrapper evidence and live-job distinction](VEHICLE_OUTER_WRAPPER.md).

11 October 2026, Europe/Kiev. The independent vehicle update now consumes authoritative linked physical pose and velocities, updates physical mass/inertia, and copies the original car control settings before the selected source-track driver. Complete original/repeat/Rust records match in debug and release, including carried synchronization → driver → linked publication. Actual world scheduling and a drivable Bevy loop remain required.

## Recovered contract

Whole `9BD7F0` calls linked getters `A69800`, `A69820` and `A69630` for physical linear velocity220, angular velocity210 and published pose1D0. These overwrite car15C0,15D0,15E0. If all three velocity components have absolute value below stored bits29E12E13 (approximately 1e-13), XYZ becomes positive zero; W is retained. Old car pose/velocity may be nonfinite because it is discarded before consumption.

The ten-entry strain ring advances modulo ten and clears its new slot. Whole `9A0A70`/`989BF0` tests its maximum: above3 enables strain; below0.5 with strain enabled clears the flag and four scores. This implementation supports **zero deformation-grid dimensions**, so no deformation-node traversal occurs. Active deformation is rejected before native execution.

Mass is sourceFA8 times scale1560. The diagonal inertia entries FAC,FB0,FB4 use the same scale; W is one. Whole `A69570→A69D40` stores its inverse using SIMD cofactors and a refined reciprocal seed. Independent `1/diagonal` divisions change stored words. The Rust diagonal specialization retains the original products, determinant, SSE reciprocal seed and refinement, including the computed W value. Positive finite mass/inertia and normal determinant are required; late inverse overflow is rejected without modifying input state.

Twenty-one floating settings copy from the car tail into the driver controls. Additional mode/assist bytes generate226C/2270/2274 and the four per-wheel flag groups. Full mappings are explicit in the authored Rust module. DB0 is zero at the pointer-free wire boundary; the native adapter supplies only an owned wrapper linking its owned physical allocation. No captured pointer is followed.

## Verification and limits

| Per debug/release profile | Calls | Complete compared bytes |
|---|---:|---:|
| Whole synchronization | 180 initial +192 carried =372 | 20,416,848 |
| Synchronization before initialized source driver | 332 | 18,221,488 |
| Selected source driver after synchronization | 140 initial +192 carried =332 | 5,107,488 |
| Linked setters/readback after each driver call | 332 | Complete physical/car records at three stages |

Standalone original/repeat/Rust comparison passes **36 guard outcomes**. Coverage includes196 changed pose/mass/inverse/settings records,372 ring changes,65 strain activations,43 resets and264 tiny-velocity cleanups. Unequal physical/car poses, varied W, signed/subthreshold/equal-threshold velocities and discarded old NaNs distinguish authoritative copying from self-copying.

The joined validation independently initializes each branch's source ratios,32-node curve,engine limit and shift thresholds, then carries its own state for twelve updates. All44 supported source tables are audited. Driver coverage includes134 source contacts/nonzero grips,1,327 spin changes,17 angular recoveries,660 camber changes and332 physical-pose/velocity/local-chassis changes. Owner settings are authored from the selected fixture; complete construction is not claimed.

Publication uses the verified F169 linked setters and getters with **no advance or correction**, empty shape lists and notification disabled. Velocity W is cleared at this outer boundary as observed in `A09A80`. All three publication snapshots consequently agree. The interframe history transfer remains authored from recovered offsets. This is a composition of verified stages, **not execution of whole `A09A80`/`A06A90`/`A03220`, world collision scheduling or a drivable lap**.

All142 library tests pass (22formats,1tools,119world), formatting, all-target warnings-denied Clippy, Python syntax and four-report audit. Four Rust borrow errors and a constant-chunk Clippy warning were fixed before successful comparisons. An inverse-overflow fixture initially expected native preflight32; its determinant remains normal and the original output triggers postflight33. Both implementations safely rejected it; the expected guard was corrected. Earlier standalone final02 copied identical poses, so final03 adds unequal poses and overwritten NaNs. No numerical mismatch or native exception occurred.

Steam executable and130 traced resources remain unchanged at **11 October 01:22:14.234622 UTC**. No desktop input, setting changes or installation occurred. Existing tools reused: Rust/Cargo1.99.0,Bevy0.19.1,Python3.12.14,MSVC14.44.35207/Developer Prompt17.14.41/SDK10.0.26100.0,Ghidra12.1.4/JDK21.0.9,Capstone5.0.6. Native strict SSE2/53-bit nearest x87 unchanged. Completion estimates remain unchanged; expired automation remains paused.

## Evidence and reproduction

Original bytes, source values, mapped executable and reports remain ignored under `local/`:

- `behavior/vehicle-sync-20261011-debug-final-03/validation.json` and release counterpart, including guards and execution hashes.
- `behavior/vehicle-linked-driver-20261011-debug-final-02/validation.json` and release counterpart, with sibling source audits and individual stage executions.
- `behavior/vehicle-sync-final-audit-20261011.json`, `vehicle-sync-library-tests-20261011.log`, `vehicle-sync-build-20261011.log`.
- `scene/preservation-20261011-vehicle-sync.json`.
- Static `vehicle-outer-sync-aligned-20261011`, `vehicle-sync-tail-aligned-20261011`, `vehicle-mass-sync-aligned-20261011`, `vehicle-mass-sync-tail-aligned-20261011`, `vehicle-sync-constants-20261011` JSON/log pairs. Successful Ghidra surveys `20261011T0104475773190Z` and `20261011T0105357232183Z`; retained `20261008T0014152064705Z/9bd7f0.c.txt` and `20261008T1311500495038Z/9a0a70.c.txt`.

Native sync adapter SHA256 `4820e0639d234e173aee167785c5df92f3c38caac1510e8e98a2aaca9438e3c1`; sync CLI debug `543510c4426c9a40b3f4e7560267960a86b65aa5f6fc4589a0ecdf484eb1c461`, release `cf9dbbab408c5a7d2a09a013279670f6deeebde628a4adf5d12c7306e289f069`. Complete dependent CLI hashes are retained in the final audit and every comparison report. Analysis PE SHA256 remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Build `tools/build_vehicle_sync_probe.cmd` and `grid-vehicle-sync`, `grid-vehicle-force`, `grid-vehicle-bridge` in both profiles. Run `tools/validate_vehicle_sync.py` and `tools/validate_vehicle_linked_driver.py` with fresh private output directories and `--profile debug|release`. VS01 holds exactD240 car bytes plus424 physical bytes,1–64 records,create-new output. The native mapper never runs entry/imports/TLS; original code remains unchanged and writable lazy-data page E5B000 is restored.

Next validate complete original `A03670` in one call against the independently composed Rust driver. Its verified chassis publication is the final call before return, making this a stronger available boundary than more isolated helper tests. Then resolve the `A06A90` tail, `A03220`, actual physical contacts/world scheduling and complete source initialization before Bevy input,camera,restart and lap checkpoints.
