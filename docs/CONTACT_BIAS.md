# Physical contact correction bias — F152

10 October 2026. Independent Rust reproduces both complete original correction helpers, **B0CE10/B0CEC0**, for **6,352 cases /317,600 output words per debug and release profile**, with zero differences against original, repeat and independent scalar expectations. Of these, 512 are authored edge cases and 5,840 are contact accumulators from fresh executions of the F151 whole serial callback chain. Correction context, initial rows and upstream owner/pending setup are authored; this is not a captured whole physical frame.

## Contract

Both helpers take the contact in ECX and one stack context argument, returning with `ret4`. They always advance their respective context row cursor by192bytes: +40 for two-body, +38 for single-body. They modify an existing constraint row; they neither detect contacts nor construct its physical Jacobian.

If contact disabled byte +0E is nonzero, they zero the first two row vectors (+40/+50); the two-body helper also zeros +60/+70. Bounds +88/+8C, bias +90 and the context's existing active flag +20 are preserved.

Otherwise they compute `bias = (contact.accumulated - context.threshold) * context.scale`, using separate f32 subtraction and multiplication. Nonnegative bias, including negative zero, sets the context active flag and retains the bias bits. Negative bias becomes positive zero and leaves the active flag as supplied. Lower bound becomes positive zero; upper bound copies global E51D40. Vectors and other row words remain unchanged.

The file-backed E51D40 value is0x7F7FFFFF, maximum finite f32 (3.4028234663852886e38); runtime mutation is unverified. Authored controls include this exact value as well as alternate supplied finite limits.

`grid_world::contact_bias::calculate` exposes this update independently of Bevy. It accepts finite accumulated/threshold/scale/upper-limit operands and requires a finite computed result when active. Existing unused row fields are preserved. The adapter checks booleans, exact framing,1–16,384cases and16MiB, writes only new private outputs and preserves opaque row bytes. These validation bounds define our supported interface, not native malformed-pointer behavior.

The [physical handoff map](PHYSICAL_CONTACT_HANDOFF.md) places these helpers inside optional B7B5E0 after impact callbacks. That establishes a downstream use of the consumer's contact+38; enabling conditions, live cadence, initial constraint creation, the full solver and final integration remain unresolved.

## Verification

`tools/probe_contact_bias.c` maps the hash-checked private analysis image, invokes each complete original function with aligned owned buffers and the recovered thiscall ABI, and compares all executable sections afterward. No imports, helper stubs, skipped instructions or code patches are needed. The supplied upper-limit global is written only in the isolated mapped image, then restored with its previous page protection. Original Steam files are never modified.

`tools/validate_contact_bias.py` verifies pinned source captures and prior F151 evidence, reconstructs its input, executes whole original serial wrapper and Rust afresh, and checks all5,918,598 upstream words. It then feeds final contact+38 into correction comparisons.1,187 handoff accumulators differ from their initial inputs. Whole correction outputs include the full192-byte row, cursor advance and active flag. An independent Python f32 scalar expectation additionally checks the contract.

Activity:1,692single/4,660two-body cases,74disabled rows,892negative and5,386nonnegative active biases,1,245negative-zero results,2,606flag activations,74vector changes and6,278bias changes. Thirteen malformed-input guards reject with ordinary exit codes below256 and no native fault marker: truncation, trailing bytes, zero/excess case count, three invalid booleans, four nonfinite operands and two overflowing computations. Rust creates no output for rejected inputs.

- Analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Original helper probe SHA256 `ed10efdda97dfe8c5b5586bd89a7ea014e754f4c09c72a5440c6c0ba113cd44b`.
- Rust debug SHA256 `868c03d08bc7b23fb5c28a547ebc314afdb1137741bee55116b4e6eda949724a`.
- Rust release SHA256 `6858cc2e3ac2e02002af85af08262205f8a5361379756eadb42f4a89696dc775`.
- Final reports: `local/behavior/contact-bias-20261010-debug-final-02/validation.json`, `contact-bias-20261010-release-final-02/validation.json`. Reports pin source captures, upstream probe and freshly built upstream Rust binaries.
- Complete aligned instructions: `contact-bias-instructions-20261010.json`, `contact-bias-tail-20261010.json`; private global inspection: `contact-bias-global-20261010.json`.

Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 and existing tools reused. Strict C build,83workspace library tests (20formats,1tools,62world), final warnings-denied all-target Clippy, formatting and Python AST parsing pass. Initial Clippy rejected a constant `chunks_exact`; changed the adapter to `as_chunks` and rebuilt/recompared both profiles. Earlier validation directories remain private. No native or numerical failures occurred in this helper comparison.

Test-count correction: F151's saved workspace run contains79library tests (20+1+58), whereas the earlier log/documentation reported112. Corrected that statement; four new behavioral boundary tests produce83now. Historical counts for other milestones are not re-audited here.

Steam130traced resources and original executable still match the baseline at2026-10-10T03:26:33.858770UTC: `local/scene/preservation-20261010-contact-bias.json`. Completion estimates remain unchanged; a drivable Rust engine is still ahead. Next substantive work is whole physical constraint construction/solver integration and synchronized vehicle initialization/timing; live owner collection awaits usable foreground control.
