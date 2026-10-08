# Handling-damage source investigation

Recorded8October2026. Current numerical checkpoint is [deterministic reset F128](STEERING.md). Source configuration below has static provenance and an independent private survey; its original parser/loader has not been executed.

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

Private aligned evidence: `handling-damage-setup-instructions-20261008.json`, `handling-damage-load-instructions-20261008.json`, `handling-damage-default-constants-20261008.json`, three handling-damage string JSONs. Targeted Ghidra output `20261008T1331402853307Z/`, `20261008T1332309391001Z/`, `20261008T1333040046986Z/`; caller survey `20261008T1336570555821Z/`. Bounded source reader and source-to-reset comparison are pending. Existing tools reused; no installation/game/settings modifications.
