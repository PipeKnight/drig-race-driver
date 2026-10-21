## Hypotheses and unresolved questions

- **H001, medium confidence:** the observed crash occurs in unsafe startup cleanup and may mask an earlier initialization failure. Do not call this the initiating root cause yet.
- Determine the first failed initialization operation before the apparent cleanup path.
- Establish whether CPU detection/worker scheduling, graphics-device creation, audio, overlays, an absent resource or another subsystem triggers that failure.
- An affinity limit may leave some CPU-count detection APIs unchanged; F008 does not rule out every CPU/worker-map defect.
- Lacetti/Donington pairing and route_0 reads are confirmed. Resolve the human-readable layout label, cached/shared dependencies and internal container consumption; do not treat the observed 130-file set as exhaustive.
- Identify the generic HID game controller and any wheel/pedal requirements when reference driving is available.
- **H002, provisional:** repeated display enumeration and/or instrumentation overhead may stretch startup well beyond the baseline. The short Procmon/native-debug probes do not establish which operation fails first.
- **H003, confirmed startup remedy in isolated copy:** process-codepage compatibility controls the observed startup failure. Paired manifest controls differ only in `en-US` versus `UTF-8` and show repeated survival versus the original exception (F018); owner confirms main menu (F019). The later `uk-UA` copy supports autosave persistence and driving. Exact initiating API/conversion and broader game-mode compatibility remain unresolved. No system-locale change/reboot made.
