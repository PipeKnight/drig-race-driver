# Handling-damage source investigation

Recorded8October2026. Current checkpoint F129 joins the native source reader to [deterministic reset F128](STEERING.md). Source mapping has aligned static provenance; the original source parser/loader is not executed.

## Native reader and reset composition (F129)

`handling_damage::read` accepts the observed ASCII HDM subset, up to64KiB/128elements/16levels/64attributes per element. It strips legacy description declarations only outside markup/attribute values, requires one VEHICLEHANDLINGDAMAGE root and one direct SUSPENSION/WHEEL each, checks matching ends/duplicate attributes and rejects malformed, ambiguous, nonfinite or oversized inputs. DOCTYPE/entity/CDATA and non-whitespace body content are unsupported. It exposes only the seven steering fields in the mapping below. Unknown attributes remain unconsumed; this is a bounded subset, not a full handling-damage reader.

`grid-steering-reset INPUT OUTPUT --source HDM` replaces all seven supplied parameter placeholders with source-derived fields. **20 datasets /2,560 cases /64,000 fields exact debug/release**:two hashed installed archive entries plus18 independently authored decimal/signed-zero/underflow/default/explicit-endpoint controls,128 supplied RNG states each with both wheel-view modes. Twenty-five compared fields comprise7parameters, final RNG state,6seeds and11 steering values/flags. Original whole9e6070->9c5b90 gets independently Python-converted parameters; native reads the HDM. **Original XML loader/configuration reader/numeric converter/full constructor remain unexecuted.** Parameter linkage is aligned static evidence. Source-mode reset does not establish current live selection, RNG state, damage evolution or scheduling.

Evidence `local/behavior/source-steering-reset-20261008-01/validation.json`,02/; original reset probe retained unchanged with F128's pinned SHA. Seven native invalid-source/reset guards pass before writing. Prior1,392-case supplied reset regresses both profiles (`steering-reset-source-regression-20261008-01/`,02/). Two new parser tests cover declarations/defaults/distinct names, every truncated prefix and malformed/ambiguous source. **99 workspace tests**, fmt and warnings-denied Clippy pass (`source-steering-reset-checks/`). Initial compilation assumed string attributes, whereas pinned API exposes bytes; corrected. Initial test literals lacked leading zero; corrected. No numerical failures.

Reused cached/pinned quick-xml0.41.0, already in Cargo.lock; builds completed offline without tool installation. Checked local API source and [official quick-xml reader documentation](https://docs.rs/quick-xml/0.41.0/quick_xml/reader/struct.Reader.html); newer documentation has different attribute types, so the local pinned API governs. Existing toolchain unchanged. Original reference preservation at13:46:41UTC confirms all130 traced resources and executable (`local/scene/preservation-20261008-handling-damage.json`). Next verify actual selected source/initialized mutable fields in a fresh reference race.

## Static source provenance

Constructor a0e880 passes body+CDF0 to whole9c6360 at a0f53e, then resets that same object via9e6070 at a0f54e. Named HANDLINGDAMAGE setup optionally chooses a custom damage filename; its fallback is cars/standard.hdm. The traced raceload.jpk contains raw standard.hdm/openwheeler.hdm and hdm_loader.xml, whose loader entries select both resources. All archive bytes match the earlier hashed race baseline. Individual Standard vehicle override selection is not yet established.

The HDM files are ASCII text with legacy `<!description>` declarations. A strict XML parser failed on these at line3; the bounded private adapter removes plain description declarations and parses the remaining XML. It rejects DOCTYPE/entity/CDATA declarations. Each HDM yields26 elements. The survey proves independent resource decoding, not original parser equivalence. `local/behavior/handling-damage-source-20261008-01/` retains the failed attempt;02/survey.json records success, entry/archive hashes and private decoded files.

9c6360 loads into object+218 via9c5750; that reader passes the same destination to9c5580. Aligned calls in9c5580 use destination+28 for suspension reader9c47a0 and destination+40 for wheel reader9c48c0. Float property reader a67b60 calls numeric converter bf3b60 and stores its x87 return to f32. Whole converter/source decoder are unexecuted here.

| Mutable object field | Node/attribute | Initial fallback |
|---|---|---|
| 250 | SUSPENSION/maxWheelLockScale | zeroed record |
| 254 | SUSPENSION/damageToStartEffect | zeroed record |
| 26C | WHEEL/maxMisalignAngle | zeroed record |
| 270 | WHEEL/maxSteerWobbleScale | zeroed record |
| 274 | WHEEL/damageToStartBaseEffect | zeroed record |
| 278 | WHEEL/damageToHaveFullEffect | stored-f32 constant0.95 |
| 27C | WHEEL/damageMinEffect | stored-f32 constant0.1 |

Wheel setup initializes its local20/24 defaults before optional property reads. Both shipped HDMs omit those two attributes. The similarly named damageToStartSteeringWobble source attribute is not read by the inspected9c48c0 path; do not substitute it for274. Outer constructor9f3d30 clears object218..37F before later setup. Fallback/override behavior beyond this inspected path remains open.

Private aligned evidence: `handling-damage-setup-instructions-20261008.json`, `handling-damage-load-instructions-20261008.json`, `handling-damage-default-constants-20261008.json`, three handling-damage string JSONs. Targeted Ghidra output `20261008T1331402853307Z/`, `20261008T1332309391001Z/`, `20261008T1333040046986Z/`; caller survey `20261008T1336570555821Z/`. Existing tools reused; no installation/game/settings modifications.
