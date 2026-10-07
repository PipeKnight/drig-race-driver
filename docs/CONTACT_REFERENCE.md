# Production wheel-contact observations

## Guarded reference (F080)

Private compatible GRID1.3 x86 process7484, start2026-10-07T11:03:04.8032772Z, executableSHA b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6. Computer Use and existing AutoHotkey2.0.29 macros selected Lacetti/Donington Grand Prix,3laps/Nevada/Normal/Random/4flashbacks. No debugger, suspension or game memory writes.

`tools/find_reference_vehicle_manager.ps1 -IncludeContacts` implies dynamics capture. It checks exact process path/start ticks/hash/module base/count-routine bytes, bounds private RW scanning to256MiB, guards manager/vector/count/backlinks, and limits each extra pointer read. Extra fields include decoded tuning, setup neighborhood, height/angle fields, five radius samples per wheel, current nonzero material records and a bounded material table. Raw values remain private. A successful read is not a synchronized simulation boundary.

First capture `local/behavior/general-vehicle-manager-20261007T1113268018652Z.json` SHAfd93eb3329f44b05b34829999c7e6d10f831ef08d6876add4171a05b01381672 finds one manager344415e0 after162,553,856 scanned bytes:12cars,303/303 successful blocks. Separate `runtime-collision-20261007T1113267099631Z.json` reports245 registry entries and107/107 blocks;8sampled shapes are type11 packed triangles. The registry count is not a triangle count.

All48 wheels have nonzero material/object/token fields at C0/C4/C8. C8 is an address-valued token, not a triangle ordinal. `9c1660` adds the selected table entry's first word to this token to obtain a material address. First entry's word64 is therefore an offset, correcting an earlier pointer interpretation. Invalid table index yields null; safe missing-material behavior has not been established for that original branch.

All sampled wheels point to the same material record5303b30. Its short runtime name is SMOOTHDRYTAR and its bump fields match both source STDR and SHTR, which share these mechanics. This cannot uniquely distinguish those source material IDs.

| Runtime offset | Observed value | Source mechanics candidate |
|---|---|---|
|90 byte|false|bumpstimebased|
|94|2.25|bumpswavelength|
|98|.01|bumpsmagnitude|
|9c|1|bumpsrealproportion|
|a0|1|bumpsproportionCarDependent|
|a4|0|Source meaning unresolved|

Player radius samples contain five angle0/base-radius pairs per wheel. Current helper consumes these values; the process that creates or changes them is separate. Damage/flatspot interpretation remains unverified.

A bounded2000ms throttle/pause macro logs normal key-up completion. Refreshed UI confirms pause. Second capture `general-vehicle-manager-20261007T1114213119655Z.json` SHAda4f5d871fb14d7d84933c7b7439aee5cf3cfa1873073d8a4f93dddcd7aaf675 reuses the guarded manager and succeeds303/303; player angles are nonzero. This is a retained-state observation, not exact calibrated integration timing.

## Cleanup and preservation

Owned game closed by Computer Use Alt-F4. Exact path/start ticks checked before stopping surviving macro28352. All17 backed-up user files compared: only display config changed. Post-config retained privately, original bytes restored SHA d222c1d1c539452335f00cad35553df18a8128e06afa3c003d8b8b36aad54463. All15save files unchanged and never overwritten. `local/reference-race/resume-20261007g/user-after-comparison.json`, `cleanup.json` record completion. At2026-10-07T16:00:18Z all130 traced resources and original executable still match, `local/scene/preservation-20261007-contact-material.json`.
