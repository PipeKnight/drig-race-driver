# Pre-solver deformation contact limit

13 October 2026. The independently authored `deformation_contact_limit::calculate` matches whole original **9A0C90** for a registered first body colliding with a static counterpart. This callback runs before the main constraint solve. It changes the impact history and contact coefficient that the later bulk deformation callback consumes; it cannot be omitted when enabling deformation.

Aligned B7B850 and fresh B78710 references establish the order: B78530 preparation (B77850 invokes 9A0C90), B7ACC0 solve, successful B78710 bulk callback/response consumption, optional B7B5E0 correction, then body publication. Appending deformation after the completed physical frame would use the wrong phase. Registered 418 and 41C callbacks have distinct roles.

## Verified behavior

The callback computes point velocity from body linear/angular velocity and the supplied center, limits relative speed for low masses, and strictly raises the selected slot in the ten-entry impact window. That history update also occurs when deformation is disabled. Corner strain gates further processing. The inverse pose maps the contact into grid coordinates; weighted nearby nonzero node markers, with nearest-node fallback, determine stiffness. A speed lookup updates damage history only when it strictly increases. The contact receives the coefficient at 40 and the callback returns a double-precision penetration-scaled impulse cap.

Operation order matters: the point transform groups `(W + Y) + (Z + X)`, and squared distance groups `(Z² + W²) + (X² + Y²)`. Rust retains those SSE operations and the observed x87 53-bit intermediate operations. This is a bounded original-output proof, not a claim about arbitrary nonfinite inputs or all floating-point implementations.

Each debug/release profile independently passes:

- 256 source-grid/authored-state cases, **11,119,616 complete output bytes**, original/repeat/Rust exact.
- 135 restricted caps, 199 changed impact windows, 108 changed lookup states, 256 changed coefficients.
- Lacetti/BMW grids, rotated/transformed poses, angular/linear velocities, low/boundary/source masses, all ten ring indices, prior windows/corners, zero/negative markers, displaced nodes including W, lookup history and disable gates.
- 20 guard cases /56 rejection or existing-output preservation outcomes.
- Read-only audit reconstructs every request and guard, verifies 71 files and five source identities per profile, current executable hashes and full output equality. Only the current ring word, A490/A494/A498 and contact40 may change; the complete linked body and other contact/descriptor words remain unchanged.

DC01 owns the complete A500 descriptor, 424-byte body, 80-byte contact and center. Pointer roles are canonical in files and relocated only into owned buffers. Original code/helpers remain unchanged; no entry point, imports or TLS are run. Seven private data pages carry supplied constants and are restored before comparing the complete mapped image. Canaries, immutable body and center are checked. Output creation never overwrites an existing file.

## Limits and next actions

Only the first-owner/static-peer branch is admitted. The other-body/wake graph, B77850 collection preparation, B78530 owner ordering and complete physical group are separate work. This module is not yet invoked by the driving runtime. The current playable build stays **c3696050d8f6**, SHA `70fdd84c5711bab4e56061e8844bef18bc523178e4d74b374d6ad3a34c45b328`.

Next prove the complete bounded preparation wrapper and compose both callbacks at their original solver phases, carrying the distinct deformation random state. Runtime source initialization must preserve the existing Standard-car descriptor/history (including A480=-1); the zero-filled cold-source proof cannot replace that persistent state. Dirty output requires wheel, collision-shape and presentation consumers; do not silently clear C7F2 to bypass guards.

## Reproduction and failures

Tools: `probe_deformation_contact_limit.c`, `build_deformation_contact_limit_probe.cmd`, `validate_deformation_contact_limit.py`, `audit_deformation_contact_limit.py`, and `grid-deformation-contact-limit`. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6. Native `/O2 /fp:strict /arch:SSE2`, x87 53-bit nearest. 196 regular libraries (25 formats +4 tools +167 world), formatting and all-target Clippy pass. No installation, desktop input, Steam/settings change or approval rejection.

Private evidence under `local/behavior/`:

| Evidence | SHA-256 |
| --- | --- |
| deformation-contact-limit-debug-20261013-final-01/validation.json | 15ba0ae2b1b05110974b5a61c892d28e7c508db5768ae7cc0e606f2807d9e622 |
| deformation-contact-limit-release-20261013-final-01/validation.json | 55ee3ddc9106bf4c65124ca45dc04f25e3188b8df8ddd0a9a448ee89eea4f086 |
| deformation-contact-limit-audit-20261013-final-01.json | ae62c5c388fa8e523f273f752e3187c8686acad486ced9c1a9bac29146808d7f |
| deformation-contact-limit-probe.exe | 474d828d1f71fd429053fbbf7e888a044d66308d2a4bd416c7a3a4859c381bd5 |
| target/debug/grid-deformation-contact-limit.exe | 5fdda3a5b125fbe066ac7ca093347c3ae2843f08b73e9785a94ea32806ae6d0a |
| target/release/grid-deformation-contact-limit.exe | fc529834a7bda79d2e5b3b22a5035dc763125ab2bb88b11718a917f145df395e |

Original PE hash `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Five input identities include this image, two verified source-grid outputs and two purchased car CSVs. Research: `20261013T1617514366319Z/`, `deformation-contact-limit-aligned-20261013.{json,txt}`, `deformation-contact-limit-constants-20261013.{json,txt}`, `deformation-callback-group-order-20261013.{json,txt}` and `20261013T1609425946860Z/`.

Retained draft01 faulted C0000005 before producing outputs: the harness attempted to supply CB3BA8 outside its first writable CB0000 page. Adding the private CB3000 data page fixed execution without changing original instructions. This is a harness failure, not an observed game compatibility failure; no fault EIP was captured. Draft02 exposed one ULP at Lacetti case31 contact40/cap, resolved by the aligned point-transform grouping above. Full draft03 and both final profiles agree exactly. The first semantic unit assertion had a factor-ten arithmetic mistake (375 instead of 3750), corrected without implementation/tolerance changes; the final regular test run passes. Failed evidence remains private.
