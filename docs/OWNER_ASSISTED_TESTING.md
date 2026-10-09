# Owner-assisted reference testing

On 9 October 2026 the owner offered to drive specific cars, complete laps or produce wall impacts when that would help the reconstruction. This is an available testing approach, not a completed experiment or a request for an immediate driving session.

Human driving is useful for positioning the car, deliberately producing contact conditions and assessing handling. Macros remain useful for repeatable input sequences. Neither method alone establishes simulation timing or numerical fidelity.

## Useful experiments

| Experiment | Question it can help answer | Preparation required |
|---|---|---|
| Straight acceleration, then braking | How do speed, gearing and body/wheel state evolve over a sequence? | Known start, assists/transmission recorded, capture of relevant inputs/state and game-time evidence |
| Steady corner, then steering release | Does steering return, understeer or body response match the reference? | Selected corner, speed range, input device and steering settings recorded |
| Low-speed frontal wall contact | Which geometric contacts and response branches occur? | Known barrier, fresh damage state and bounded contact/response capture armed |
| Glancing side impact | How do contact ordering, rotation and damage differ from a frontal contact? | Separate repeat from a known initial state; appropriate capture armed |
| A complete Lacetti/Donington lap | What behavior should the first Rust milestone reproduce, and where does it feel wrong? | Consistent race setup; trajectory/timing capture where available; later comparable Rust build for feedback |

The first targeted session should address a concrete missing dependency, such as geometric contact production or an active collision branch. A general lap can provide context, but it cannot substitute for synchronized telemetry when testing numerical behavior. We do not yet have an integrated Rust car for an original-versus-replacement driving comparison.

## Session protocol

1. Identify one research question. Prepare and verify the recorder before asking the owner to drive. If capture cannot be verified, report that and do not count the run as successful evidence.
2. Send one short instruction: car/track, starting location/state, assists/transmission, maneuver and stopping condition. Use the existing Chevrolet Lacetti/Donington reference unless another choice answers the question better. Specify speed units explicitly if a speed target is needed.
3. Hand input control to the owner for the run. Do not send macros or desktop input while the owner is driving. Establish a clear handoff before automation resumes.
4. Capture only what the current tooling supports, and record any missing inputs, timing or state. Debugger pauses can distort wall-clock input duration; use game-time/tick evidence when available and do not infer it from human timing or macro logs.
5. Ask the owner to pause after the maneuver. Verify recorder completion and inspect the result before requesting another run. Restart/reset between impact tests so previous damage does not silently change the conditions; record unknown reset state.
6. Repeat a short maneuver when useful to measure reference variability. Record the owner's observations separately from measured data, including input device and settings. A perfect lap is unnecessary.

Keep proprietary recordings and runtime state under ignored `local/`; publish independently authored methodology and findings in Markdown. Link each successful or failed session from [PROJECT_LOG.md](../PROJECT_LOG.md), with the executable/resource identity, setup, evidence paths, limitations and next question.

No immediate owner action is required. Offline integration work can continue. When owner help becomes useful, provide an armed capture and a precise maneuver rather than asking for unguided driving.
