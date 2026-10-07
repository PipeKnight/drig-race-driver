# Running vehicle-state investigation

## Scope and failed interface hypothesis

The reference is the private Cyrillic-compatible GRID executable, PID 19680, started at `2026-10-06T01:04:50.0562006Z`. The reader verifies its path, executable hash, start time, module base and live vehicle-count instructions against the private unpacked analysis image. It opens the process only for query/read access. No input, injection, memory write or suspension is involved; snapshots are not atomic.

The original named-plugin lookup uses registry base `0x00e59148`, 32-byte name slots and a separate pointer array. Looking up `VehicleManagerPlugin` returns `0x00e3b058` in this process. The count/index getters use fields `this+0x4c` and `this+0x50` as a pointer-vector range. Applying that layout directly to the named plugin was a failed hypothesis: both fields contain integer 12. The vector guard stopped before following any car pointer.

The first failed run lacked a persisted diagnostic record. The reader was corrected to preserve already-read bounded blocks on failure; the repeated guarded attempt confirms the mismatch. Evidence: `local/behavior/runtime-vehicles-failed-20261006T1456290297484Z.json`, `runtime-vehicle-state-preserved-run.txt`. This establishes a wrong interface view, not a corrupt vehicle vector. A possible adjusted/nested manager remains to be verified from original instructions before another vehicle read.

The plugin's first virtual method at `0x00742c40` returns `0x00e6e834`, used by the original type-validation path. That global is not established as a vehicle-manager pointer. The earlier collision snapshot's `vehicle_manager_cache` block name was speculative and must not be treated as its semantics.

Static evidence: `local/behavior/20261006T1459088526226Z/`, `20261006T1459183780195Z/`, `20261006T1503339290679Z/`. Research continues with actual manager accessor call sites; wheel/frame data are not yet captured.

## Separate indexed component manager

Plugin creation `0x008f2e70` allocates a separate `0x5760`-byte manager and stores its pointer at plugin `+0x7c`. The consumer `0x0064b100` reads count `+0x598`; getter `0x0099c050` indexes twenty pointers at `+0x12a0`. Its actual instructions and the general count routine match the running process.

The resulting bounded snapshot confirms twelve populated indexed components, consistent with the observed twelve-car race. All 56 requested blocks were readable. Applying the wheel layout directly to those components produces zero wheel counts; this is another object-view mismatch, not evidence that cars lack wheels. The indexed object has vtable `0x00d0dea0`; its constructor and links are being traced. Evidence: `local/behavior/runtime-vehicles-20261006T1510571650914Z.json`, `runtime-physics-vehicle-state-run.txt`.

The general vehicle-manager pointer is created by `0x006326a0`, stored at central object `+0x218c`, and uses a separate `0x13a0`-byte allocation. The original spawn path demonstrably follows a general vehicle's `+0x24` body pointer before reading wheels and dimensions. These distinct manager/component/body views must remain separate in our specification.

A static survey failed when an unquoted hexadecimal address was coerced by PowerShell into a number; retry with quoted strings succeeded. The wrapper now rejects invalid address strings before launching Ghidra. A subsequent store scan rejected a relative project path; retry with an absolute path succeeded. Neither failure changed the running game or source files. New private evidence includes `vehicle-core-manager-stores.tsv`, `vehicle-core-manager-chain-corrected-run.txt`, and `vehicle-body-spawn-instructions.json`.

The indexed component's base constructor stores an argument at `+0x18`. Treating that link as a general car was also rejected by a guard: the pointed object has vtable `0x00d466d0` and a pointer-valued `+0x6c` field. No body was followed from this rejected view. Evidence: `runtime-vehicles-failed-20261006T1516074610437Z.json`. Inspection of the actual spawn instructions also clarifies that kinds 3 and 4 are **excluded**, rather than selected, by `0x00916f20`. The successful manager capture below supersedes these incorrect view hypotheses.

## General manager and body state verified

Constructor `0x00a11ca0` installs general-manager vtable `0x00d199cc`. A bounded read-only search of committed private writable memory found no candidate in its first 64 MiB. Expanding the hard limit to 256 MiB found exactly one candidate after reading **215,076,864 bytes**, across 364 eligible regions. Only candidate fields and metadata were retained; unmatched heap contents were discarded. No mapped/image, executable or guard pages were searched. Candidate `0x344415e0` matches the constructor vtable and count/index layout; its vector contains twelve distinct general car pointers.

Each car's `+0x24` body pointer is distinct and exposes **four wheels**, with stride `0x150` at body `+0x1990`. Grid indices form exactly the permutation 0..11. Kind values are zero for vector ordinal 0 and one for all eleven others. Getter `0x009ee380(0)` explicitly selects the first kind-zero car; that is stronger evidence than assuming vector order identifies the locally controlled vehicle. Kind-one AI semantics remain an inference.

| Observed field | Original consumer / validation |
|---|---|
| Manager `+0x4c`, `+0x50` | Pointer-vector begin/end, count routine `0x009ad030` |
| General car `+0x24` | Body pointer followed by spawn builder `0x00915270` |
| General car `+0x6c` | Kind selector in `0x009ee380`; excluded kinds 3/4 in grid placement |
| General car `+0x2e4c`, `+0x2e50` | Grid index and adjacent mode field consumed by placement |
| Body `+0x1620` | Wheel count read by ground-aligned spawn setter |
| Body `+0x1990 + wheel*0x150` | Wheel record, including initial four-float vector |
| Wheel `+0x40` | Addend to the vector's Y component in `0x009b62e0` |
| Wheel `+0xe0` | Clearance comparison operand in `0x009de4e0`; final force meaning pending |
| Body `+0x1630` | Four-float offset vector used by dimension getter |
| Body `+0xed64`, `+0xed68`, `+0xed6c`, `+0xed88`, `+0xed70` | Dimension-getter operands; physical bounds/units still need source linkage |

The kind-zero car is assigned grid index 2. Its captured body vector at `+0x1610` is approximately (-182.31218, -11.834136, 19.551117, 1). This is compatible with the Donington grid area and subsequent brief driving, but is not a fresh starting pose or an independently established ride height. Selected frame vectors and all 48 wheel base vectors are finite. Do not infer suspension forces, contact state or timing from these fields alone.

The manager can now be read directly with `tools/find_reference_vehicle_manager.ps1 -ManagerAddress 0x344415e0`; exact process/start/path/hash/module/code/vtable guards apply before following the vector. This address belongs only to this reference process and is not an engine constant. The first direct-mode attempt finished reads but failed while formatting absent scan metadata; corrected nullable metadata handling before successful retry.

Two successful snapshots at `2026-10-06T15:21:04Z` and `15:22:51Z` contain **75/75 identical captured blocks**, twelve cars and 48 wheel records. `tools/validate_vehicle_state.py` checks block lengths/read success, vtable, uniqueness, grid permutation, count, finite selected vectors and bitwise snapshot stability. Unsuspended reads are not atomic; matching selected fields does not prove full-state determinism or a simulation tick rate. There was no new game input in this continuation.

Evidence: `local/behavior/general-vehicle-manager-20261006T1521045126136Z.json`, `general-vehicle-manager-20261006T1522510770722Z.json`, `vehicle-state-validation.json`, search/direct run logs, original accessor survey `20261006T1522511529141Z/`. Use the general-manager reader for body research; the older `read_reference_vehicle_state.ps1` records the separate indexed-component investigation and its rejected owner hypothesis.

## Timed throttle response and tuning linkage

At 15:31 UTC Computer Use finds the original window minimized, restores it through the approved API, and directly observes Pause / Continue. The already-running verified AutoHotkey runner handles one F5 sample at `15:31:53..15:31:55Z`: resume pulse 156 ms, throttle requested/reported **2000 ms**, final pause pulse 172 ms, all key-up logged. An immediate UI capture shows 44 mph, gear 2 and time 00:34.21; because that capture occurs during the input sequence, it is not the final speed measurement. A subsequent capture verifies Pause / Continue again.

Before/after guarded snapshots show the kind-zero body's vector at `+0x1610` changing from approximately (-182.31218, -11.834136, 19.551117) to (-234.06837, -12.990731, 10.092182). Its vector at `+0x15c0` changes from (-19.610355, -0.39928043, -3.5784442) to (-35.691006, -0.8606982, -6.5243435). These are observed field changes correlated with original throttle input; exact motion/velocity semantics and game-time intervals remain to be traced. Wall-clock timestamps and macro duration are not a simulation-tick measurement. No crash or stuck-key condition observed.

Evidence: `general-vehicle-manager-20261006T1530061504882Z.json`, `general-vehicle-manager-20261006T1532098370009Z.json`, `vehicle-throttle-response-validation.json`, existing runner log `local/macros/20261006T0915102081230Z-serve.tsv`. Expanded captures contain 111 readable blocks. Original window remains paused; no settings/profile changes requested.

Body and wheel numeric copy consumers are now identified and compared against original CSVs and Rust output: [body parameters](BODY_PARAMETERS.md). This is the supported bridge from source resources to live physical parameters; complete driving/suspension reconstruction remains ahead.

## Linked pose and velocity storage

Disassembly of spawn setter `0x009ef040` supplies body `+0xdb0` as the receiver for `0x00a696f0`, `0x00a69840` and `0x00a69860`. Those routines follow wrapper `+8` into a separate state object. Pose getter `0x00a69630` copies its 64-byte matrix at `+0x1d0`. Setter `0x00a696f0` delegates to `0x00b12710`, which stores that pose and constructs another matrix at `+0x190`, incorporating offset `+0x230` into the translation. The latter is not assumed to be identical in every configuration.

Setter `0x00a69840` copies the four-float speed-times-forward vector into linked state `+0x220`; `0x00a69860` copies the other four-vector to `+0x210`, cleared by the spawn sequence. These are the linear and angular velocity storage candidates supported by the setter chain. Full update/units/timing remain under investigation. The body has corresponding pose at `+0x15e0`, linear vector at `+0x15c0` and angular vector at `+0x15d0`.

An expanded read-only capture at `15:47:47Z` reads **147/147 blocks**. All twelve body poses are bit-identical to linked `+0x1d0` matrices: **192 exact f32 words**. Eleven cars also have identical linear and angular four-vectors. Car ordinal 10 differs slightly in both vectors (largest absolute differences approximately 0.00000763 linear and 0.00000780 angular). These differences are retained; unsuspended sequential reads and the separate views do not establish their cause or exact update synchronization. The kind-zero car matches all compared words.

`tools/validate_vehicle_linked_state.py` independently checks address-chain correspondence, successful lengths, finite selected fields and exact/different values. Private evidence: `general-vehicle-manager-20261006T1547472612827Z.json`, `linked-vehicle-state-validation.json`, `spawn-body-setter-instructions.json`, original routines under `20261006T1546156162090Z/`, `20261006T1546418914946Z/` and `20261006T1548417753432Z/`.

The older indexed-component reader now stops at the verified component header and records its database link without following the rejected owner/body hypothesis. The general-manager reader now preserves failed read evidence before closing its read-only handle.
