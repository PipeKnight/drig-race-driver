### 6 October 2026 — current reconstruction roadmap

- Owner asked whether we have a plan for decompilation and a Rust or C++ rewrite. Reviewed the original proposal against verified installation, toolchain and menu-startup results.
- Created [current implementation plan](docs/IMPLEMENTATION_PLAN.md), preserving original phase numbering and explicitly separating complete identification, partial toolchain setup, menu access and pending driving validation.
- Recorded targeted decompilation workflow: locate code, observe runtime inputs/outputs, specify verified structures/behavior, implement independently, and compare. Full binary-to-source recovery is not required before the first parser/viewer.
- Rust/Bevy remains the recommended default under the original request; C++ is a viable alternative, without assuming it reduces reverse-engineering uncertainty. Keep resource specifications and simulation independent of the renderer; pin actual compiler/dependency versions when first building.
- Checked official Bevy/Rust setup documentation for the development prerequisite plan. No new tools installed or engine code added this turn. First deliverables remain a verified reference race/dependency map, bounded resource inspector, scene viewer, and eventually one timed lap.
