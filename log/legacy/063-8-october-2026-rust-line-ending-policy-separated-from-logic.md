### 8 October 2026 — Rust line-ending policy separated from logic

- Commit8d90460 adds RustLFpolicy in.gitattributes and normalizes three existing CRLF/mixed files (contact_material.rs, contact_ray.rs, grid-worldlib.rs). StagedRustdiffwithignore-space-at-eol exitszero: no Rustcontent/tokenchanges. Separated from F112physics to keep reviews focused; preceding95workspacechecks pass. Restoring F112module initiallyplaced it aftersuspension; finalfmtcheck foundorderingonly, cargo fmt corrected beforecommit andfinalcheckpasses. No tools/game/userfilechanges.
