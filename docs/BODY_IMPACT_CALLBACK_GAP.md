# Body-impact damage callback connection

F248's scheduled Donington tests complete with physical body responses but no global damage RNG change. Their sampled wheel-impact operands stay below the wheel threshold. Static original-code research identifies a separate route that must be recovered before claiming general body-impact damage.

Observed Ghidra references to damage dispatcher `9E6660` identify two direct callers: handling update `9F3E80` (now integrated) and `9EF360` at `9EF438` (not integrated). The latter is called by `A01910` at `A01951`. An earlier original decompilation and fresh aligned instructions establish that `9EF360` checks event40 against maximum-float sentinel, clears car1568 and BA5, optionally flips all four direction sign bits for side1, and calls `9E6660` with event point10, direction20 and globalE4FC30 times event40. Original data wordE4FC30 is1.0 in the analyzed image; this does not prove its live value or semantic units. The returned byte is stored in BA5.

For the observed `A01910` call, the last argument is0. The following original handler also updates suspension-related state, queries the physical body, visits optional callbacks and contains further event/notification branches. Porting only its damage call would omit those effects. No whole-handler execution/equivalence or runtime body-damage integration is claimed here.

The original contact-pool consumer `A726B0` reads each shape's21C callback registration, builds an event through `B0CFC0`, deduplicates equal function/user pairs and invokes the selected callbacks. `A01910` selects the side whose registered physical owner matches carDB0 and calls `9EF360`. This clarifies the gap in the current callback-free driving world; the older [whole physical world proof](WHOLE_PHYSICAL_WORLD.md) explicitly excludes registered callbacks.

Fresh decompilation of observed `B0CFC0` shows a small event builder: contact50/60 become event10/20, contact3C becomes event40, contact38 becomes event4C, contact30/34 become event8/C, and event30..3C/44/48 are zeroed. Event0/4 are preserved caller-supplied owner registrations. This is currently an observation from pseudocode, pending aligned/native complete-output verification. Solver writeback already updates contact3C from its solved previous-impulse field, so the fresh-contact maximum-float placeholder is not assumed to survive solving.

Private evidence:

- `local/behavior/20261012T2239451461241Z/{address-refs,contexts}.tsv`: the two dispatcher callers.
- `20261012T2241064838883Z/{address-refs,contexts}.tsv`: `9EF360` caller `A01910`.
- `20261012T2242213243474Z/b0cfc0.c.txt`: newly observed event builder.
- `body-impact-event-prefix-20261013-draft-01.{json,txt}` and `body-impact-event-constants-20261013-draft-01.json`: aligned original prefix and bounded data reads.
- Reused `20261007T0827311096024Z/9ef360.c.txt`, `20261012T0302461545897Z/a01910.c.txt`, `20261011T1433384595310Z/a726b0.c.txt`.
- `body-damage-caller-survey-20261013-draft-01.txt`: sandboxed launcher failed saving Java path in AppData, despite installed JDK. Authorized escalated retry `body-damage-caller-survey-20261013-final-01.txt` succeeds; subsequent caller/builder surveys succeed. This was a filesystem restriction, not an automatic approval rejection or missing-tool problem.

Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused, analysis image SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. All original/derived code remains private. Next verify the event ABI and whole builder/consumer on owned registered contexts, then recover the reached whole handler and source registrations before changing driving behavior. No owner capture is currently required.

## Aligned ABI follow-up

Subsequent aligned inspection confirms the builder mapping above, with two numerical details for the eventual native proof: point/direction are copied with aligned SIMD moves, while contact38/3C pass through x87 load/store. A raw-bit copy is therefore not assumed equivalent for every NaN payload. Event storage and contact storage require the observed alignment; the builder preserves event0/4.

`A726B0` takes three stack arguments and returns with12-byte stack cleanup. The consumer does not use an incoming object register for its own state. Its registered callbacks use caller cleanup, receiving event pointer, the third consumer argument as an x87-loaded/stored float, the second consumer argument as a raw word, and user data. Original `A6A270` retrieves callback/user from registrationC/10. Equal callback and user pairs are deduplicated; otherwise eligible sides run in order. These are observed ABI facts, pending owned native execution.

`A01910` selects the first side whose `A6A1D0` result equals carDB0. The getter follows registration8, then that object'sD8, then a424 link when non-null. This is not a direct registration8-to-car-owner equality. Class meanings and source registration ownership remain unverified; native fixtures must own the complete reached chain rather than reinterpret numeric pose words as pointers. The handler receives last argument0, so proving its wheel-mode early exit alone would not cover the actual body event.

Aligned private evidence: `local/behavior/body-impact-event-abi-20261013-draft-01.{json,txt}`, `body-impact-event-side-owner-20261013-draft-01.{json,txt}`, `body-impact-event-owner-getter-20261013-draft-01.{json,txt}`. No new runtime implementation or native equivalence is claimed. One guessed native-probe filename read failed harmlessly and was corrected with inventory. The executable and purchased files remain unchanged.
